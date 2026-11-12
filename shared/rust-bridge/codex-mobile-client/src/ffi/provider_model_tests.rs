use super::*;
use crate::session::connection::{ServerConfig, ServerSession, TestRequestHandler};
use std::sync::Mutex;

#[tokio::test]
async fn provider_catalog_paginates_without_replacing_default_models() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let handler: TestRequestHandler = Arc::new(move |request| {
        seen.lock()
            .unwrap()
            .push(serde_json::to_value(&request).unwrap());
        match request {
            upstream::ClientRequest::ModelProviderList { .. } => {
                Ok(serde_json::json!({"data":[{"id":"custom"}]}))
            }
            upstream::ClientRequest::ModelList { params, .. } => {
                if params
                    .model_provider
                    .as_deref()
                    .is_some_and(|id| id.starts_with("litter-provider-capability-probe-"))
                {
                    return Err(crate::transport::RpcError::ServerWithData {
                        code: -32602,
                        message: "not configured".into(),
                        data: serde_json::json!({"type":"modelProviderNotConfigured","modelProvider":params.model_provider}),
                    });
                }
                assert_eq!(params.model_provider.as_deref(), Some("custom"));
                let first = params.cursor.is_none();
                Ok(
                    serde_json::json!({"data":[model(if first {"one"} else {"two"})],"nextCursor":if first {Some("page2")} else {None}}),
                )
            }
            _ => panic!("unexpected request"),
        }
    });
    let (client, app) = app(handler);
    client.app_store.update_server_models(
        "s",
        Some(vec![types::ModelInfo::from(
            serde_json::from_value::<upstream::Model>(model("cached-default")).unwrap(),
        )]),
    );
    let before = client.app_snapshot().servers["s"].available_models.clone();
    let models = app
        .list_models_for_provider("s".into(), "custom".into())
        .await
        .unwrap();
    assert_eq!(
        models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert!(
        models
            .iter()
            .all(|m| m.provider_id.as_deref() == Some("custom") && !m.picker_name.is_empty())
    );
    assert_eq!(client.app_snapshot().servers["s"].available_models, before);
    assert_eq!(requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn provider_catalog_rejects_old_servers_unknown_providers_and_cursor_loops() {
    for mode in [
        "old",
        "unknown",
        "loop",
        "discovery-only",
        "unrelated-error",
    ] {
        let handler: TestRequestHandler = Arc::new(move |request| match request {
            upstream::ClientRequest::ModelProviderList { .. } if mode == "old" => {
                Err(crate::transport::RpcError::Server {
                    code: -32601,
                    message: "method not found".into(),
                })
            }
            upstream::ClientRequest::ModelProviderList { .. } => Ok(
                serde_json::json!({"data":if mode == "unknown" {vec![]} else {vec![serde_json::json!({"id":"custom"})]}}),
            ),
            upstream::ClientRequest::ModelList { params, .. }
                if params
                    .model_provider
                    .as_deref()
                    .is_some_and(|id| id.starts_with("litter-provider-capability-probe-")) =>
            {
                if mode == "discovery-only" {
                    return Ok(serde_json::json!({"data":[],"nextCursor":null}));
                }
                let error_type = if mode == "unrelated-error" {
                    "authenticationFailed"
                } else {
                    "modelProviderNotConfigured"
                };
                Err(crate::transport::RpcError::ServerWithData {
                    code: -32602,
                    message: "probe rejected".into(),
                    data: serde_json::json!({"type":error_type,"modelProvider":params.model_provider}),
                })
            }
            upstream::ClientRequest::ModelList { .. } if mode == "loop" => {
                Ok(serde_json::json!({"data":[],"nextCursor":"same"}))
            }
            _ => panic!("must not fall back to the default provider"),
        });
        let (_, app) = app(handler);
        assert!(
            app.list_models_for_provider("s".into(), "custom".into())
                .await
                .is_err()
        );
    }
}

fn app(handler: TestRequestHandler) -> (Arc<MobileClient>, AppClient) {
    let client = Arc::new(MobileClient::new());
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
        .upsert_server(&config, crate::store::ServerHealthSnapshot::Connected);
    let session = Arc::new(ServerSession::test_stub_with_handlers(
        config,
        Some(handler),
        None,
        None,
    ));
    client.sessions.write().unwrap().insert("s".into(), session);
    let app = AppClient {
        inner: client.clone(),
        rt: crate::ffi::shared::shared_runtime(),
    };
    (client, app)
}

fn model(id: &str) -> serde_json::Value {
    serde_json::json!({"id":id,"model":id,"displayName":id,"description":"Example", "hidden":false,
        "supportedReasoningEfforts":[],"defaultReasoningEffort":"medium","isDefault":false})
}

#[tokio::test]
async fn managed_provider_probe_can_prove_selector_support_without_bypassing_policy() {
    let handler: TestRequestHandler = Arc::new(|request| match request {
        upstream::ClientRequest::ModelProviderList { .. } => {
            Ok(serde_json::json!({"data":[{"id":"custom"}]}))
        }
        upstream::ClientRequest::ModelList { params, .. }
            if params.model_provider.as_deref() == Some("custom") =>
        {
            Ok(serde_json::json!({"data":[model("allowed")],"nextCursor":null}))
        }
        upstream::ClientRequest::ModelList { params, .. } => {
            Err(crate::transport::RpcError::ServerWithData {
                code: -32602,
                message: "policy conflict".into(),
                data: serde_json::json!({"type":"modelProviderPolicyConflict","modelProvider":params.model_provider,"requiredModelProvider":"custom"}),
            })
        }
        _ => panic!("unexpected request"),
    });
    let (_, app) = app(handler);
    assert_eq!(
        app.list_models_for_provider("s".into(), "custom".into())
            .await
            .unwrap()[0]
            .id,
        "allowed"
    );
}

#[tokio::test]
async fn reconnect_between_probe_and_model_response_cannot_return_an_old_servers_default_catalog() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        for step in 0..5 {
            let raw = socket.next().await.unwrap().unwrap();
            let request: serde_json::Value = serde_json::from_str(raw.to_text().unwrap()).unwrap();
            let response = match step {
                0 => serde_json::json!({"id":request["id"],"result":{}}),
                1 => {
                    assert_eq!(request["method"], "initialized");
                    continue;
                }
                2 => serde_json::json!({"id":request["id"],"result":{"data":[{"id":"custom"}]}}),
                3 => {
                    serde_json::json!({"id":request["id"],"error":{"code":-32602,"message":"not configured","data":{"type":"modelProviderNotConfigured","modelProvider":request["params"]["modelProvider"]}}})
                }
                _ => {
                    assert_eq!(request["params"]["modelProvider"], "custom");
                    break;
                }
            };
            socket
                .send(Message::Text(response.to_string().into()))
                .await
                .unwrap();
        }
        drop(socket); // Admission/query reply is lost, forcing the same session to reconnect.
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        let request: serde_json::Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({"id":request["id"],"result":{}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        let _initialized = socket.next().await.unwrap().unwrap();
        let request: serde_json::Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["method"], "model/list");
        socket.send(Message::Text(serde_json::json!({"id":request["id"],"result":{"data":[model("wrong-default")],"nextCursor":null}}).to_string().into())).await.unwrap();
        let _ = released.await;
    });
    let client = Arc::new(MobileClient::new());
    let config = ServerConfig {
        server_id: "s".into(),
        display_name: "Server".into(),
        host: "127.0.0.1".into(),
        port: address.port(),
        websocket_url: Some(format!("ws://{address}/rpc")),
        is_local: false,
        tls: false,
    };
    client
        .app_store
        .upsert_server(&config, crate::store::ServerHealthSnapshot::Connected);
    let session = Arc::new(ServerSession::connect_remote(config).await.unwrap());
    client
        .sessions
        .write()
        .unwrap()
        .insert("s".into(), session.clone());
    let app = AppClient {
        inner: client,
        rt: crate::ffi::shared::shared_runtime(),
    };
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        app.list_models_for_provider("s".into(), "custom".into()),
    )
    .await
    .unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Connection changed")
    );
    let _ = release.send(());
    session.disconnect().await;
    server.await.unwrap();
}
