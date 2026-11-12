//! Project asynchronous questions onto the existing mobile question UI.
//! Pending answers survive turn completion so mobile can answer after backgrounding.

use super::*;

impl AppStoreReducer {
    pub(crate) fn clear_async_questions_for_prompt(
        &self,
        key: &ThreadKey,
        input: &[upstream::UserInput],
    ) {
        if crate::async_questions::parse_input(input).is_some() {
            return;
        }
        let requests = {
            let snapshot = self.snapshot.read().expect("app store lock poisoned");
            snapshot
                .pending_user_inputs
                .iter()
                .filter(|r| {
                    r.is_async && r.server_id == key.server_id && r.thread_id == key.thread_id
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        for request in requests {
            self.resolve_async_question(&request);
        }
    }

    pub(crate) fn resolve_async_question(&self, request: &PendingUserInputRequest) {
        let key = ThreadKey {
            server_id: request.server_id.clone(),
            thread_id: request.thread_id.clone(),
        };
        let requests = {
            let mut snapshot = self.write_snapshot();
            snapshot
                .resolved_async_questions
                .entry(key.clone())
                .or_default()
                .extend(request.questions.iter().map(|q| q.id.clone()));
            snapshot.pending_user_inputs.retain(|r| r.id != request.id);
            snapshot.pending_user_inputs.clone()
        };
        self.emit(AppStoreUpdateRecord::PendingUserInputsChanged { requests });
        self.emit_thread_metadata_changed(&key);
    }

    pub(super) fn reconcile_async_questions(&self, key: &ThreadKey) {
        let Some(thread) = self.thread_snapshot(key) else {
            return;
        };
        // Resolve replies first: history pages may arrive before the source question.
        for item in thread.items.iter().filter(|item| {
            matches!(
                item.content,
                HydratedConversationItemContent::UserInputResponse(_)
            )
        }) {
            self.apply_async_question_item(key, item, false);
        }
        // A newer ordinary prompt supersedes older questions, including ones
        // retained locally after their source turn finished. Do not let an old
        // history page clear questions whose source is outside that page.
        let positions = thread
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.as_str(), i))
            .collect::<HashMap<_, _>>();
        let last_prompt = thread
            .items
            .iter()
            .rposition(|item| matches!(item.content, HydratedConversationItemContent::User(_)));
        if let Some(last_prompt) = last_prompt {
            let requests = self
                .snapshot()
                .pending_user_inputs
                .into_iter()
                .filter(|r| {
                    r.is_async
                        && r.server_id == key.server_id
                        && r.thread_id == key.thread_id
                        && positions
                            .get(r.item_id.as_str())
                            .is_some_and(|index| *index < last_prompt)
                })
                .collect::<Vec<_>>();
            for request in requests {
                self.resolve_async_question(&request);
            }
        }
        for (index, item) in thread.items.iter().enumerate().filter(|(_, item)| {
            matches!(item.content, HydratedConversationItemContent::Assistant(_))
        }) {
            if last_prompt.is_some_and(|prompt| index < prompt) {
                continue;
            }
            let live = match &thread.active_turn_id {
                Some(turn_id) => item.source_turn_id.as_ref() == Some(turn_id),
                // Paged hydration can arrive before the active turn-id probe.
                // Closed turns have their actionable metadata stripped by hydration.
                None => thread.info.status == ThreadSummaryStatus::Active,
            };
            self.apply_async_question_item(key, item, live);
        }
    }

    pub(super) fn apply_async_question_item(
        &self,
        key: &ThreadKey,
        item: &HydratedConversationItem,
        live: bool,
    ) {
        match &item.content {
            HydratedConversationItemContent::Assistant(data)
                if live && data.async_questions.as_ref().is_some_and(|q| !q.is_empty()) => {}
            HydratedConversationItemContent::User(_) if live => {}
            HydratedConversationItemContent::UserInputResponse(_) => {}
            _ => return,
        }
        let changed = {
            let mut snapshot = self.write_snapshot();
            let before = snapshot.pending_user_inputs.clone();
            match &item.content {
                HydratedConversationItemContent::UserInputResponse(data) => {
                    snapshot
                        .resolved_async_questions
                        .entry(key.clone())
                        .or_default()
                        .extend(data.questions.iter().map(|q| q.id.clone()));
                    let resolved = snapshot
                        .resolved_async_questions
                        .get(key)
                        .cloned()
                        .unwrap_or_default();
                    snapshot.pending_user_inputs.retain(|r| {
                        !r.is_async
                            || r.server_id != key.server_id
                            || r.thread_id != key.thread_id
                            || !r
                                .questions
                                .iter()
                                .any(|q| resolved.contains(&q.id) || resolved.contains(&r.item_id))
                    });
                }
                HydratedConversationItemContent::User(_) if live => {
                    let thread = snapshot.threads.get(key);
                    let prompt_position = thread
                        .and_then(|t| t.items.iter().position(|existing| existing.id == item.id));
                    let superseded = |r: &&PendingUserInputRequest| {
                        r.is_async
                            && r.server_id == key.server_id
                            && r.thread_id == key.thread_id
                            && prompt_position.is_none_or(|prompt| {
                                thread
                                    .and_then(|t| {
                                        t.items.iter().position(|source| source.id == r.item_id)
                                    })
                                    .is_some_and(|source| source < prompt)
                            })
                    };
                    let ids = snapshot
                        .pending_user_inputs
                        .iter()
                        .filter(superseded)
                        .flat_map(|r| r.questions.iter().map(|q| q.id.clone()))
                        .collect::<Vec<_>>();
                    snapshot
                        .resolved_async_questions
                        .entry(key.clone())
                        .or_default()
                        .extend(ids.clone());
                    snapshot.pending_user_inputs.retain(|r| {
                        !r.is_async
                            || r.server_id != key.server_id
                            || r.thread_id != key.thread_id
                            || !r.questions.iter().any(|q| ids.contains(&q.id))
                    });
                }
                HydratedConversationItemContent::Assistant(data) if live => {
                    let info = snapshot.threads.get(key).map(|t| t.info.clone());
                    for question in data.async_questions.iter().flatten() {
                        if snapshot
                            .resolved_async_questions
                            .get(key)
                            .is_some_and(|ids| ids.contains(&question.id) || ids.contains(&item.id))
                        {
                            continue;
                        }
                        let id = serde_json::json!([
                            "async-question",
                            key.server_id,
                            key.thread_id,
                            question.id
                        ])
                        .to_string();
                        if snapshot.pending_user_inputs.iter().any(|r| r.id == id) {
                            continue;
                        }
                        snapshot.pending_user_inputs.push(PendingUserInputRequest {
                            is_async: true,
                            id,
                            server_id: key.server_id.clone(),
                            thread_id: key.thread_id.clone(),
                            turn_id: item.source_turn_id.clone().unwrap_or_default(),
                            item_id: item.id.clone(),
                            questions: vec![question.clone()],
                            requester_agent_nickname: info
                                .as_ref()
                                .and_then(|i| i.agent_nickname.clone()),
                            requester_agent_role: info.as_ref().and_then(|i| i.agent_role.clone()),
                        });
                    }
                }
                _ => {}
            }
            (before != snapshot.pending_user_inputs).then(|| snapshot.pending_user_inputs.clone())
        };
        if let Some(requests) = changed {
            self.emit(AppStoreUpdateRecord::PendingUserInputsChanged { requests });
            self.emit_thread_metadata_changed(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(id: &str) -> HydratedConversationItem {
        crate::conversation::hydrate_thread_item(
            &upstream::ThreadItem::UserMessage {
                id: id.into(),
                client_id: None,
                content: vec![upstream::UserInput::Text {
                    text: "New task".into(),
                    text_elements: vec![],
                }],
            },
            Some("turn"),
            None,
            &Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn authoritative_history_resolves_only_questions_before_a_new_prompt() {
        let store = AppStoreReducer::new();
        let mut current = thread("s");
        current.items.push(question());
        store.upsert_thread_snapshot(current.clone());
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        current.items.push(prompt("new-task"));
        let mut later = question();
        later.id = "later".into();
        if let HydratedConversationItemContent::Assistant(data) = &mut later.content {
            data.async_questions.as_mut().unwrap()[0].id = "later-question".into();
        }
        current.items.push(later);
        store.upsert_thread_snapshot(current.clone());
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        assert_eq!(store.snapshot().pending_user_inputs[0].item_id, "later");
        // Repeated user-item completion must not dismiss a newer question.
        store.apply_item_update(&current.key, prompt("new-task"));
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        store.upsert_thread_snapshot(current);
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
    }

    #[test]
    fn session_replacement_removes_blocking_requests_but_preserves_async_and_other_servers() {
        let store = AppStoreReducer::new();
        let mut current = thread("s");
        current.items.push(question());
        store.upsert_thread_snapshot(current);
        let asynchronous = store.snapshot().pending_user_inputs[0].clone();
        let mut blocking = asynchronous.clone();
        blocking.id = "blocking".into();
        blocking.is_async = false;
        let mut other = blocking.clone();
        other.server_id = "other".into();
        other.id = "other-input".into();
        store.replace_pending_user_inputs(vec![asynchronous.clone(), blocking, other]);
        let approval = crate::types::PendingApproval {
            id: "approval".into(),
            server_id: "s".into(),
            kind: crate::types::ApprovalKind::Command,
            thread_id: Some("t".into()),
            turn_id: None,
            item_id: None,
            command: None,
            path: None,
            grant_root: None,
            cwd: None,
            reason: None,
        };
        let mut other = approval.clone();
        other.id = "other-approval".into();
        other.server_id = "other".into();
        store.replace_pending_approvals(vec![approval, other]);
        store.invalidate_connection_requests("s");
        let snapshot = store.snapshot();
        assert_eq!(snapshot.pending_user_inputs.len(), 2);
        assert_eq!(snapshot.pending_user_inputs[0], asynchronous);
        assert_eq!(snapshot.pending_approvals.len(), 1);
        assert_eq!(snapshot.pending_approvals[0].server_id, "other");
    }

    #[test]
    fn older_history_pages_do_not_resolve_a_question_outside_the_page() {
        let store = AppStoreReducer::new();
        let mut current = thread("s");
        current.items.push(question());
        store.upsert_thread_snapshot(current.clone());
        current.items = vec![prompt("older")].into();
        store.upsert_thread_snapshot(current);
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
    }

    fn thread(server: &str) -> ThreadSnapshot {
        let info = serde_json::from_value(
            serde_json::json!({"id":"t", "status":ThreadSummaryStatus::Active}),
        )
        .unwrap();
        let mut thread = ThreadSnapshot::from_info(server, info);
        thread.active_turn_id = Some("turn".into());
        thread
    }

    fn question() -> HydratedConversationItem {
        crate::conversation::hydrate_thread_item(
            &upstream::ThreadItem::AgentMessage {
                id: "q".into(),
                text: "Which?\n- A\n- B".into(),
                phase: None,
                memory_citation: None,
                delivery: Some(upstream::AgentMessageDelivery::Async),
                questions: Some(vec![upstream::AsyncUserInputQuestion {
                    title: "Which?".into(),
                    options: Some(vec!["A".into(), "B".into()]),
                }]),
            },
            Some("turn"),
            None,
            &Default::default(),
        )
        .unwrap()
    }

    #[test]
    fn async_questions_replay_skip_and_other_clients_do_not_reopen_questions() {
        let store = AppStoreReducer::new();
        let mut first = thread("s1");
        first.items.push(question());
        let key = first.key.clone();
        store.upsert_thread_snapshot(first.clone());
        store.upsert_thread_snapshot(first.clone());
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        let request = store.snapshot().pending_user_inputs[0].clone();
        assert!(request.is_async);
        assert!(request.questions[0].is_other_allowed);
        store.resolve_async_question(&request);
        store.upsert_thread_snapshot(first.clone());
        assert!(store.snapshot().pending_user_inputs.is_empty());
        let mut second = thread("s2");
        second.items.push(question());
        store.upsert_thread_snapshot(second);
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        // A desktop reply can arrive before its question's history page.
        let reply = crate::async_questions::reply_text(
            &request,
            &[PendingUserInputAnswer {
                question_id: request.questions[0].id.clone(),
                answers: vec!["A".into()],
            }],
        )
        .unwrap();
        let user = upstream::ThreadItem::UserMessage {
            client_id: None,
            id: "reply".into(),
            content: vec![upstream::UserInput::Text {
                text: reply,
                text_elements: vec![],
            }],
        };
        let item = crate::conversation::hydrate_thread_item(
            &user,
            Some("turn"),
            None,
            &Default::default(),
        )
        .unwrap();
        let other = AppStoreReducer::new();
        other.upsert_thread_snapshot(thread("s1"));
        other.apply_item_update(&key, item);
        other.upsert_thread_snapshot(first);
        assert!(other.snapshot().pending_user_inputs.is_empty());
    }

    #[test]
    fn async_questions_keep_pending_answers_at_turn_end_but_ignore_closed_history() {
        let store = AppStoreReducer::new();
        let mut current = thread("s");
        let key = current.key.clone();
        current.items.push(question());
        store.upsert_thread_snapshot(current.clone());
        current.active_turn_id = None;
        current.info.status = ThreadSummaryStatus::Idle;
        store.upsert_thread_snapshot(current.clone());
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
        let fresh = AppStoreReducer::new();
        fresh.upsert_thread_snapshot(current);
        assert!(fresh.snapshot().pending_user_inputs.is_empty());
        let prompt = crate::conversation::hydrate_thread_item(
            &upstream::ThreadItem::UserMessage {
                client_id: None,
                id: "new-prompt".into(),
                content: vec![upstream::UserInput::Text {
                    text: "New task".into(),
                    text_elements: vec![],
                }],
            },
            Some("next"),
            None,
            &Default::default(),
        )
        .unwrap();
        store.apply_item_update(&key, prompt);
        assert!(store.snapshot().pending_user_inputs.is_empty());
    }

    #[test]
    fn async_questions_paged_history_restores_running_questions_before_turn_id_probe() {
        let raw = upstream::ThreadItem::AgentMessage {
            id: "q".into(),
            text: "Which?".into(),
            phase: None,
            memory_citation: None,
            delivery: Some(upstream::AgentMessageDelivery::Async),
            questions: Some(vec![upstream::AsyncUserInputQuestion {
                title: "Which?".into(),
                options: None,
            }]),
        };
        let mut turn = upstream::Turn {
            id: "turn".into(),
            items: vec![raw],
            items_view: Default::default(),
            status: upstream::TurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };
        let closed = crate::conversation::hydrate_turns(&[turn.clone()], &Default::default());
        assert!(
            matches!(&closed[0].content, HydratedConversationItemContent::Assistant(data) if data.async_questions.is_none())
        );
        turn.status = upstream::TurnStatus::InProgress;
        let running = crate::conversation::hydrate_turns(&[turn], &Default::default());
        let mut current = thread("s");
        current.active_turn_id = None;
        current.items = running.into();
        let store = AppStoreReducer::new();
        store.upsert_thread_snapshot(current);
        assert_eq!(store.snapshot().pending_user_inputs.len(), 1);
    }
}
