//! UI request identities retain the capability from the connection that emitted them.

use super::*;
use codex_app_server_client::AppServerReplyHandle;
use codex_app_server_protocol::{JSONRPCRequest, ServerNotification};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct ServerReplies {
    wire_ids: HashMap<RequestId, RequestId>,
    handles: HashMap<RequestId, AppServerReplyHandle>,
    thread_ids: HashMap<RequestId, String>,
    completed: HashSet<RequestId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_client::{JsonRpcWire, RemoteAppServerClient};
    use codex_app_server_protocol::{JSONRPCMessage, JSONRPCResponse};

    struct Wire {
        messages: mpsc::UnboundedSender<JSONRPCMessage>,
        responses: mpsc::UnboundedReceiver<JSONRPCMessage>,
    }
    impl JsonRpcWire for Wire {
        async fn send_message(&mut self, message: JSONRPCMessage, _: &str) -> std::io::Result<()> {
            self.messages.send(message).map_err(std::io::Error::other)
        }
        async fn next_message(&mut self, _: &str) -> std::io::Result<Option<JSONRPCMessage>> {
            Ok(self.responses.recv().await)
        }
        async fn close(&mut self, _: &str) -> std::io::Result<()> {
            Ok(())
        }
    }

    async fn client() -> AppServerClient {
        let (messages, mut requests) = mpsc::unbounded_channel();
        let (responses, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(message) = requests.recv().await {
                if let JSONRPCMessage::Request(request) = message {
                    responses
                        .send(JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result: serde_json::json!({}),
                        }))
                        .unwrap();
                }
            }
        });
        AppServerClient::Remote(
            RemoteAppServerClient::connect_with_wire(
                RemoteAppServerConnectArgs {
                    endpoint: RemoteAppServerEndpoint::WebSocket {
                        websocket_url: "ws://localhost".into(),
                        auth_token: None,
                    },
                    client_name: "test".into(),
                    client_version: "1".into(),
                    experimental_api: true,
                    mcp_server_openai_form_elicitation: false,
                    opt_out_notification_methods: vec![],
                    channel_capacity: 16,
                },
                "test".into(),
                Wire {
                    messages,
                    responses: rx,
                },
            )
            .await
            .unwrap(),
        )
    }

    fn request() -> AppServerEvent {
        AppServerEvent::ServerRequest(Box::new(serde_json::from_value(serde_json::json!({
            "id":7,"method":"item/tool/requestUserInput","params":{"threadId":"t","turnId":"turn","itemId":"item","questions":[]}
        })).unwrap()))
    }

    fn id(event: AppServerEvent) -> RequestId {
        let AppServerEvent::ServerRequest(request) = event else {
            panic!("expected request")
        };
        request.id().clone()
    }

    #[tokio::test]
    async fn reconnect_id_reuse_cannot_resolve_an_old_question() {
        let first = client().await;
        let mut replies = ServerReplies::default();
        let old_id = id(replies.bind(request(), &first).unwrap());
        let retained = replies.take(&old_id).unwrap();
        assert!(replies.take(&old_id).is_err());
        let invalidated = replies.clear();
        assert!(
            matches!(&invalidated[..], [ServerNotification::ServerRequestResolved(resolved)] if resolved.request_id == old_id && resolved.thread_id == "t")
        );
        first.shutdown().await.unwrap();
        let second = client().await;
        let new_id = id(replies.bind(request(), &second).unwrap());
        assert_ne!(old_id, new_id);
        assert!(retained.resolve(serde_json::json!({})).await.is_err());
        assert!(replies.take(&old_id).is_err());
        let resolved = AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ServerRequestResolved(
                codex_app_server_protocol::ServerRequestResolvedNotification {
                    thread_id: "t".into(),
                    request_id: RequestId::Integer(7),
                },
            ),
        ));
        let AppServerEvent::ServerNotification(notification) =
            replies.bind(resolved, &second).unwrap()
        else {
            panic!("expected notification")
        };
        let ServerNotification::ServerRequestResolved(resolved) = *notification else {
            panic!("expected resolved")
        };
        assert_eq!(resolved.request_id, new_id);
        assert!(replies.take(&new_id).is_err());
        second.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn duplicate_wire_requests_keep_identity_and_do_not_restore_a_consumed_reply() {
        let client = client().await;
        let mut replies = ServerReplies::default();
        let local = id(replies.bind(request(), &client).unwrap());
        assert_eq!(id(replies.bind(request(), &client).unwrap()), local);
        let _reply = replies.take(&local).unwrap();
        assert_eq!(id(replies.bind(request(), &client).unwrap()), local);
        assert!(replies.take(&local).is_err());
        replies.complete(&local);
        let replacement = id(replies.bind(request(), &client).unwrap());
        assert_ne!(replacement, local);
        let (events, mut received) = broadcast::channel(4);
        invalidate_server_replies(&mut replies, &events, &"pi".into());
        let mut invalidated = HashSet::new();
        for _ in 0..2 {
            let ServerEvent::Notification {
                runtime_kind,
                notification: ServerNotification::ServerRequestResolved(resolved),
            } = received.recv().await.unwrap()
            else {
                panic!("expected invalidation")
            };
            assert_eq!(runtime_kind, "pi");
            invalidated.insert(resolved.request_id);
        }
        assert_eq!(invalidated, HashSet::from([local, replacement]));
        assert!(replies.clear().is_empty());
        client.shutdown().await.unwrap();
    }
}

impl ServerReplies {
    pub(super) fn complete(&mut self, local_id: &RequestId) {
        self.handles.remove(local_id);
        // Retain notification/expiry identity even if the awaiting UI task was
        // cancelled after the write. A later wire-ID reuse gets a fresh identity.
        self.completed.insert(local_id.clone());
    }

    pub(super) fn clear(&mut self) -> Vec<ServerNotification> {
        self.wire_ids.clear();
        self.handles.clear();
        self.completed.clear();
        self.thread_ids
            .drain()
            .map(|(request_id, thread_id)| {
                ServerNotification::ServerRequestResolved(
                    codex_app_server_protocol::ServerRequestResolvedNotification {
                        thread_id,
                        request_id,
                    },
                )
            })
            .collect()
    }

    pub(super) fn bind(
        &mut self,
        mut event: AppServerEvent,
        client: &AppServerClient,
    ) -> std::io::Result<AppServerEvent> {
        match &mut event {
            AppServerEvent::ServerRequest(request) => {
                let wire_id = request.id().clone();
                let local_id = match self.wire_ids.get(&wire_id) {
                    Some(id) if !self.completed.contains(id) => id.clone(),
                    _ => {
                        let id = RequestId::String(format!(
                            "litter-server-request-{}",
                            crate::next_request_id()
                        ));
                        self.wire_ids.insert(wire_id.clone(), id.clone());
                        id
                    }
                };
                let mut rpc: JSONRPCRequest = serde_json::from_value(
                    serde_json::to_value(request.as_ref()).map_err(std::io::Error::other)?,
                )
                .map_err(std::io::Error::other)?;
                // Keep identity and single-use completion stable if a peer repeats a request.
                if !self.thread_ids.contains_key(&local_id) {
                    self.thread_ids.insert(
                        local_id.clone(),
                        rpc.params
                            .as_ref()
                            .and_then(|p| p.get("threadId"))
                            .and_then(|id| id.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    );
                    self.handles.insert(
                        local_id.clone(),
                        client.request_handle().server_request_reply_handle(wire_id),
                    );
                }
                rpc.id = local_id;
                **request = ServerRequest::try_from(rpc).map_err(std::io::Error::other)?;
            }
            AppServerEvent::ServerNotification(notification) => {
                if let ServerNotification::ServerRequestResolved(resolved) = notification.as_mut() {
                    if let Some(local_id) = self.wire_ids.remove(&resolved.request_id) {
                        self.handles.remove(&local_id);
                        self.thread_ids.remove(&local_id);
                        self.completed.remove(&local_id);
                        resolved.request_id = local_id;
                    }
                }
            }
            _ => {}
        }
        Ok(event)
    }

    pub(super) fn take(
        &mut self,
        request_id: &RequestId,
    ) -> Result<AppServerReplyHandle, RpcError> {
        self.handles.remove(request_id).ok_or_else(|| RpcError::Deserialization(
            "This request is no longer active on its original connection. Refresh before responding.".into()
        ))
    }
}
