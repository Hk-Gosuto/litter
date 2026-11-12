use super::*;
use crate::session::connection::TestRequestHandler;
use std::sync::atomic::{AtomicBool, Ordering};

#[tokio::test]
async fn async_question_answers_steer_without_rpc_response_and_retain_failed_requests() {
    let client = MobileClient::new();
    let config = ServerConfig {
        server_id: "s".into(),
        display_name: "Server".into(),
        host: "localhost".into(),
        port: 8390,
        websocket_url: None,
        is_local: false,
        tls: false,
    };
    client
        .app_store
        .upsert_server(&config, ServerHealthSnapshot::Connected);
    let info = serde_json::from_value(
        serde_json::json!({"id":"t", "status":crate::types::ThreadSummaryStatus::Active}),
    )
    .unwrap();
    let mut thread = ThreadSnapshot::from_info("s", info);
    thread.active_turn_id = Some("turn".into());
    let key = thread.key.clone();
    client.app_store.upsert_thread_snapshot(thread);
    let question = crate::async_questions::questions(
        "q",
        &[upstream::AsyncUserInputQuestion {
            title: "Which?".into(),
            options: Some(vec!["A".into()]),
        }],
    );
    let request = PendingUserInputRequest {
        id: "async-request".into(),
        is_async: true,
        server_id: "s".into(),
        thread_id: "t".into(),
        turn_id: "turn".into(),
        item_id: "q".into(),
        questions: question,
        requester_agent_nickname: None,
        requester_agent_role: None,
    };
    client
        .app_store
        .replace_pending_user_inputs(vec![request.clone()]);
    let fail = Arc::new(AtomicBool::new(true));
    let received = Arc::new(StdMutex::new(Vec::new()));
    let handler: TestRequestHandler = {
        let fail = fail.clone();
        let received = received.clone();
        Arc::new(move |rpc| {
            let upstream::ClientRequest::TurnSteer { params, .. } = rpc else {
                panic!("expected turn/steer")
            };
            assert_eq!(params.expected_turn_id, "turn");
            received.lock().unwrap().push(params.input);
            if fail.load(Ordering::SeqCst) {
                Err(RpcError::Deserialization("send failed".into()))
            } else {
                Ok(serde_json::json!({"turnId":"turn"}))
            }
        })
    };
    let session = Arc::new(ServerSession::test_stub_with_handlers(
        config,
        Some(handler),
        Some(Arc::new(|_, _| {
            panic!("async questions must not respond to a server RPC")
        })),
        None,
    ));
    client.sessions.write().unwrap().insert("s".into(), session);
    let answers = vec![PendingUserInputAnswer {
        question_id: request.questions[0].id.clone(),
        answers: vec!["custom answer".into()],
    }];
    assert!(
        client
            .respond_to_user_input(&request.id, answers.clone())
            .await
            .is_err()
    );
    assert_eq!(client.app_store.snapshot().pending_user_inputs.len(), 1);
    fail.store(false, Ordering::SeqCst);
    client
        .respond_to_user_input(&request.id, answers)
        .await
        .unwrap();
    assert!(client.app_store.snapshot().pending_user_inputs.is_empty());
    let input = received.lock().unwrap()[1].clone();
    assert_eq!(
        crate::async_questions::parse_input(&input).unwrap()[0].answer,
        "custom answer"
    );
    assert!(
        client
            .app_store
            .thread_snapshot(&key)
            .unwrap()
            .queued_follow_ups
            .is_empty()
    );
    // Skipping is local and does not steer the model.
    client
        .app_store
        .replace_pending_user_inputs(vec![request.clone()]);
    client
        .respond_to_user_input(&request.id, Vec::new())
        .await
        .unwrap();
    assert!(client.app_store.snapshot().pending_user_inputs.is_empty());
}

#[tokio::test]
async fn async_question_answer_after_turn_end_starts_a_turn_with_readable_optimistic_echo() {
    let client = MobileClient::new();
    let config = ServerConfig {
        server_id: "s".into(),
        display_name: "Server".into(),
        host: "localhost".into(),
        port: 8390,
        websocket_url: None,
        is_local: false,
        tls: false,
    };
    client
        .app_store
        .upsert_server(&config, ServerHealthSnapshot::Connected);
    let info = serde_json::from_value(
        serde_json::json!({"id":"t", "status":crate::types::ThreadSummaryStatus::Idle}),
    )
    .unwrap();
    let thread = ThreadSnapshot::from_info("s", info);
    let key = thread.key.clone();
    client.app_store.upsert_thread_snapshot(thread);
    let request = PendingUserInputRequest {
        id: "async-request".into(),
        is_async: true,
        server_id: "s".into(),
        thread_id: "t".into(),
        turn_id: "previous".into(),
        item_id: "q".into(),
        questions: crate::async_questions::questions(
            "q",
            &[upstream::AsyncUserInputQuestion {
                title: "Which?".into(),
                options: None,
            }],
        ),
        requester_agent_nickname: None,
        requester_agent_role: None,
    };
    client
        .app_store
        .replace_pending_user_inputs(vec![request.clone()]);
    let handler: TestRequestHandler = Arc::new(|rpc| {
        let upstream::ClientRequest::TurnStart { params, .. } = rpc else {
            panic!("expected turn/start")
        };
        assert_eq!(
            crate::async_questions::parse_input(&params.input).unwrap()[0].answer,
            "After completion"
        );
        Ok(serde_json::json!({"turn":{"id":"next", "items":[], "status":"inProgress"}}))
    });
    let session = Arc::new(ServerSession::test_stub_with_handlers(
        config,
        Some(handler),
        None,
        None,
    ));
    client.sessions.write().unwrap().insert("s".into(), session);
    client
        .respond_to_user_input(
            &request.id,
            vec![PendingUserInputAnswer {
                question_id: request.questions[0].id.clone(),
                answers: vec!["After completion".into()],
            }],
        )
        .await
        .unwrap();
    assert!(client.app_store.snapshot().pending_user_inputs.is_empty());
    let thread = client.app_store.thread_snapshot(&key).unwrap();
    let item = thread.local_overlay_items.first().unwrap();
    assert!(matches!(
        item.content,
        crate::conversation_uniffi::HydratedConversationItemContent::UserInputResponse(_)
    ));
    assert_eq!(item.source_turn_id.as_deref(), Some("next"));
    let echo = crate::conversation::hydrate_thread_item(
        &upstream::ThreadItem::UserMessage {
            client_id: None,
            id: "server-echo".into(),
            content: vec![upstream::UserInput::Text {
                text: crate::async_questions::reply_text(
                    &request,
                    &[PendingUserInputAnswer {
                        question_id: request.questions[0].id.clone(),
                        answers: vec!["After completion".into()],
                    }],
                )
                .unwrap(),
                text_elements: vec![],
            }],
        },
        Some("next"),
        None,
        &Default::default(),
    )
    .unwrap();
    // The authoritative echo replaces the optimistic answer, rather than duplicating it.
    client.app_store.apply_ui_event(&UiEvent::ItemCompleted {
        key: key.clone(),
        notification: upstream::ItemCompletedNotification {
            completed_at_ms: 0,
            thread_id: "t".into(),
            turn_id: "next".into(),
            item: upstream::ThreadItem::UserMessage {
                client_id: None,
                id: "server-echo".into(),
                content: vec![upstream::UserInput::Text {
                    text: crate::async_questions::reply_text(
                        &request,
                        &[PendingUserInputAnswer {
                            question_id: request.questions[0].id.clone(),
                            answers: vec!["After completion".into()],
                        }],
                    )
                    .unwrap(),
                    text_elements: vec![],
                }],
            },
        },
    });
    assert!(
        client
            .app_store
            .thread_snapshot(&key)
            .unwrap()
            .local_overlay_items
            .is_empty()
    );
    assert!(matches!(
        echo.content,
        crate::conversation_uniffi::HydratedConversationItemContent::UserInputResponse(_)
    ));
}
