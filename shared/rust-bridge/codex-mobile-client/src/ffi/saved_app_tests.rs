use super::*;
use crate::saved_apps::*;
use crate::session::connection::{ServerConfig, ServerSession, TestRequestHandler};
use crate::store::ServerHealthSnapshot;
use serde_json::{Value, json};
use std::sync::Mutex;

fn start_response() -> Value {
    json!({
        "thread": {"id":"edit", "preview":"", "ephemeral":true,
            "modelProvider":"openai", "createdAt":1, "updatedAt":1,
            "status":{"type":"idle"}, "cwd":"/remote", "cliVersion":"test",
            "source":"appServer", "turns":[]},
        "model":"test-model", "modelProvider":"openai", "cwd":"/remote",
        "approvalPolicy":"on-request", "approvalsReviewer":"user", "sandbox":{"type":"readOnly"}
    })
}

fn attach(client: &MobileClient, id: &str, runtime: &str, handler: TestRequestHandler) {
    let config = ServerConfig {
        server_id: id.into(),
        display_name: id.into(),
        host: "remote.test".into(),
        port: 8390,
        websocket_url: None,
        is_local: false,
        tls: false,
    };
    client
        .app_store
        .upsert_server(&config, ServerHealthSnapshot::Connected);
    let session =
        ServerSession::test_stub_with_runtime_handlers(config, vec![(runtime.into(), handler)]);
    client
        .sessions
        .write()
        .unwrap()
        .insert(id.into(), Arc::new(session));
}

fn app(directory: &str, server: Option<&str>) -> SavedApp {
    saved_app_upsert_for_server(
        directory.into(),
        server.map(str::to_string),
        "origin".into(),
        "counter".into(),
        "Counter".into(),
        "<p>old</p>".into(),
        640.0,
        480.0,
        2,
    )
    .unwrap()
}

#[tokio::test]
async fn remote_thread_start_registers_widget_tools_without_losing_custom_tools() {
    let client = Arc::new(MobileClient::new());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    attach(
        &client,
        "remote",
        "codex",
        Arc::new(move |request| {
            if let upstream::ClientRequest::ThreadStart { params, .. } = request {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::to_value(params).unwrap());
                Ok(start_response())
            } else {
                panic!("unexpected request")
            }
        }),
    );
    let ffi = AppClient {
        inner: client,
        rt: shared_runtime(),
    };
    let params = types::AppStartThreadRequest {
        agent_runtime_kind: Some("codex".into()),
        model: None,
        cwd: None,
        approval_policy: None,
        sandbox: None,
        developer_instructions: Some("keep my instructions".into()),
        persist_extended_history: true,
        ephemeral: None,
        dynamic_tools: Some(vec![types::AppDynamicToolSpec {
            name: "custom".into(),
            description: "custom".into(),
            input_schema_json: "{\"type\":\"object\"}".into(),
            defer_loading: false,
        }]),
    };
    ffi.start_thread("remote".into(), params).await.unwrap();
    let requests = requests.lock().unwrap();
    let tools = requests[0]["dynamicTools"].as_array().unwrap();
    for name in ["show_widget", "visualize_read_me", "custom"] {
        assert_eq!(tools.iter().filter(|tool| tool["name"] == name).count(), 1);
    }
    assert!(
        requests[0]["developerInstructions"]
            .as_str()
            .unwrap()
            .contains("keep my instructions")
    );
}

#[test]
fn widget_registration_skips_bridge_and_ephemeral_threads() {
    for (runtime, ephemeral) in [("pi", None), ("local-studio", None), ("codex", Some(true))] {
        let mut params: types::AppStartThreadRequest = serde_json::from_value(json!({
            "persistExtendedHistory":true, "ephemeral":ephemeral
        }))
        .unwrap();
        crate::widget_guidelines::register_thread_tools(runtime, &mut params);
        assert!(params.dynamic_tools.is_none());
    }
}

#[test]
fn saved_apps_are_scoped_by_server_and_old_records_still_load() {
    let dir = tempfile::tempdir().unwrap();
    let directory = dir.path().to_str().unwrap();
    let legacy = app(directory, None);
    let a = app(directory, Some("a"));
    let b = app(directory, Some("b"));
    assert_ne!(a.id, b.id);
    assert_ne!(legacy.id, a.id);
    assert_eq!(a.app_id, b.app_id);
    saved_app_save_state(directory.into(), a.id.clone(), "{\"count\":7}".into(), 2).unwrap();
    let updated = saved_app_upsert_for_server(
        directory.into(),
        Some("a".into()),
        "origin".into(),
        "counter".into(),
        "A".into(),
        "<p>new A</p>".into(),
        640.0,
        480.0,
        2,
    )
    .unwrap();
    assert_eq!(updated.id, a.id);
    assert_eq!(
        saved_app_get(directory.into(), a.id)
            .unwrap()
            .state_json
            .as_deref(),
        Some("{\"count\":7}")
    );
    assert_eq!(
        saved_app_get(directory.into(), b.id).unwrap().widget_html,
        "<p>old</p>"
    );
    let mut old_json = serde_json::to_value(&legacy).unwrap();
    old_json.as_object_mut().unwrap().remove("origin_server_id");
    let decoded: SavedApp = serde_json::from_value(old_json).unwrap();
    assert_eq!(decoded, legacy);
}

#[tokio::test]
async fn saved_app_server_selection_prefers_origin_and_ignores_offline_or_bridge_hosts() {
    let client = MobileClient::new();
    let dir = tempfile::tempdir().unwrap();
    let saved = app(dir.path().to_str().unwrap(), Some("z-origin"));
    for (id, runtime) in [("a-other", "codex"), ("z-origin", "codex"), ("0-pi", "pi")] {
        attach(
            &client,
            id,
            runtime,
            Arc::new(|_| panic!("selection must not call RPC")),
        );
    }
    assert_eq!(
        choose_saved_app_server_id(&client, &saved).as_deref(),
        Some("z-origin")
    );
    client.disconnect_server("z-origin");
    assert_eq!(
        choose_saved_app_server_id(&client, &saved).as_deref(),
        Some("a-other")
    );
    client.disconnect_server("a-other");
    assert!(choose_saved_app_server_id(&client, &saved).is_none());
}

fn emit_message(client: &MobileClient, server: &str, turn: &str, text: &str) {
    let n: upstream::ItemCompletedNotification = serde_json::from_value(json!({
        "threadId":"edit", "turnId":turn, "completedAtMs":1,
        "item":{"type":"agentMessage","id":"message","text":text}
    }))
    .unwrap();
    client.event_processor.process_notification(
        server,
        "codex".into(),
        &upstream::ServerNotification::ItemCompleted(n),
    );
}

async fn update_scenario(
    output: &str,
    fail_turn: bool,
) -> (Result<SavedApp, String>, SavedAppWithPayload, Vec<Value>) {
    let client = Arc::new(MobileClient::new());
    let dir = tempfile::tempdir().unwrap();
    let directory = dir.path().to_str().unwrap().to_owned();
    let saved = app(&directory, Some("remote"));
    saved_app_save_state(
        directory.clone(),
        saved.id.clone(),
        "{\"count\":42}".into(),
        2,
    )
    .unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let weak = Arc::downgrade(&client);
    let output = output.to_string();
    attach(
        &client,
        "remote",
        "codex",
        Arc::new(move |request| {
            captured
                .lock()
                .unwrap()
                .push(serde_json::to_value(&request).unwrap());
            match request {
                upstream::ClientRequest::ThreadStart { params, .. } => {
                    assert!(params.cwd.is_none());
                    assert_eq!(params.ephemeral, Some(true));
                    assert!(matches!(
                        params.sandbox,
                        Some(upstream::SandboxMode::ReadOnly)
                    ));
                    assert!(params.dynamic_tools.is_none());
                    Ok(start_response())
                }
                upstream::ClientRequest::TurnStart { params, .. } => {
                    assert!(params.output_schema.is_some());
                    let client = weak.upgrade().unwrap();
                    // Wrong-server and previous-turn events must not contaminate
                    // the current response, even when their thread IDs match.
                    emit_message(&client, "remote", "turn", &output);
                    emit_message(
                        &client,
                        "different-server",
                        "turn",
                        "{\"widget_html\":\"wrong server\"}",
                    );
                    emit_message(
                        &client,
                        "remote",
                        "older-turn",
                        "{\"widget_html\":\"wrong turn\"}",
                    );
                    let error = fail_turn
                        .then(|| json!({"message":"host failed", "additionalDetails":null}));
                    let turn = json!({"id":"turn","items":[],"status":if fail_turn {"failed"} else {"completed"},"error":error});
                    let n: upstream::TurnCompletedNotification =
                        serde_json::from_value(json!({"threadId":"edit","turn":turn})).unwrap();
                    client.event_processor.process_notification(
                        "remote",
                        "codex".into(),
                        &upstream::ServerNotification::TurnCompleted(n),
                    );
                    Ok(json!({"turn":{"id":"turn","items":[],"status":"inProgress","error":null}}))
                }
                upstream::ClientRequest::ThreadArchive { .. } => Ok(json!({})),
                _ => panic!("unexpected request"),
            }
        }),
    );
    let result =
        update_saved_app_remotely(&client, "remote", &directory, &saved.id, "make it blue").await;
    let payload = saved_app_get(directory.clone(), saved.id).unwrap();
    let requests = requests.lock().unwrap().clone();
    assert_eq!(requests.last().unwrap()["method"], "thread/archive");
    let turn_request = requests
        .iter()
        .find(|r| r["method"] == "turn/start")
        .unwrap();
    let input: Value =
        serde_json::from_str(turn_request["params"]["input"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(input["current_html"], "<p>old</p>");
    assert_eq!(input["requested_change"], "make it blue");
    assert!(
        !serde_json::to_string(&requests)
            .unwrap()
            .contains(&directory)
    );
    (result, payload, requests)
}

#[tokio::test]
async fn remote_update_returns_html_and_preserves_user_state_and_provenance() {
    let (result, payload, _) = update_scenario("{\"widget_html\":\"<p>blue</p>\"}", false).await;
    let updated = result.unwrap();
    assert_eq!(payload.widget_html, "<p>blue</p>");
    assert_eq!(payload.state_json.as_deref(), Some("{\"count\":42}"));
    assert_eq!(updated.origin_server_id.as_deref(), Some("remote"));
    assert_eq!(updated.origin_thread_id.as_deref(), Some("origin"));
    assert_eq!(updated.schema_version, 2);
    assert_eq!((updated.width, updated.height), (640.0, 480.0));
}

#[tokio::test]
async fn remote_update_failure_or_invalid_html_leaves_saved_app_untouched() {
    for (output, failed) in [
        ("not JSON", false),
        ("{\"widget_html\":\" \"}", false),
        ("{\"widget_html\":\"new\"}", true),
    ] {
        let (result, payload, _) = update_scenario(output, failed).await;
        assert!(result.is_err());
        assert_eq!(payload.widget_html, "<p>old</p>");
        assert_eq!(payload.state_json.as_deref(), Some("{\"count\":42}"));
    }
}
