//! Remote app-server client, adapted from OpenAI Codex rust-v0.162.0-alpha.4.
//! Apache-2.0; see LICENSE. Embedded app-server startup is deliberately excluded.
mod path;
mod remote;
use codex_app_server_protocol::{
    ClientNotification, ClientRequest, JSONRPCErrorError, RequestId, Result as JsonRpcResult,
    ServerNotification, ServerRequest,
};
use codex_utils_absolute_path::AbsolutePathBuf;
pub use path::AppServerPath;
pub use remote::{
    JsonRpcWire, RemoteAppServerClient, RemoteAppServerConnectArgs, RemoteAppServerEndpoint,
};
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fmt;
use std::io::{Error as IoError, Result as IoResult};
use std::time::Duration;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
pub type RequestResult = std::result::Result<JsonRpcResult, JSONRPCErrorError>;

#[derive(Debug, Clone)]
pub enum AppServerEvent {
    Lagged { skipped: usize },
    ServerNotification(Box<ServerNotification>),
    ServerRequest(Box<ServerRequest>),
    Disconnected { message: String },
}

/// Layered error for [`AppServerRequestHandle::request_typed`].
///
/// This keeps transport failures, server-side JSON-RPC failures, and response
/// decode failures distinct so callers can decide whether to retry, surface a
/// server error, or treat the response as an internal request/response mismatch.
#[derive(Debug)]
pub enum TypedRequestError {
    Transport {
        method: String,
        source: IoError,
    },
    Server {
        method: String,
        source: JSONRPCErrorError,
    },
    Deserialize {
        method: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for TypedRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { method, source } => {
                write!(f, "{method} transport error: {source}")
            }
            Self::Server { method, source } => {
                write!(
                    f,
                    "{method} failed: {} (code {})",
                    source.message, source.code
                )?;
                if let Some(data) = source.data.as_ref() {
                    write!(f, ", data: {data}")?;
                }
                Ok(())
            }
            Self::Deserialize { method, source } => {
                write!(f, "{method} response decode error: {source}")
            }
        }
    }
}

impl Error for TypedRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport { source, .. } => Some(source),
            Self::Server { .. } => None,
            Self::Deserialize { source, .. } => Some(source),
        }
    }
}

// Await transport in a separate statement before calling this: dropping the
// completed future must precede any caller-provided Deserialize implementation.
fn decode_typed_response<T>(
    method: &str,
    response: IoResult<RequestResult>,
) -> Result<T, TypedRequestError>
where
    T: DeserializeOwned,
{
    let response = response.map_err(|source| TypedRequestError::Transport {
        method: method.to_string(),
        source,
    })?;
    let result = response.map_err(|source| TypedRequestError::Server {
        method: method.to_string(),
        source,
    })?;
    serde_json::from_value(result).map_err(|source| TypedRequestError::Deserialize {
        method: method.to_string(),
        source,
    })
}

#[derive(Clone)]
pub enum AppServerRequestHandle {
    Remote(remote::RemoteAppServerRequestHandle),
}
pub enum AppServerClient {
    Remote(RemoteAppServerClient),
}
impl AppServerRequestHandle {
    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        match self {
            Self::Remote(handle) => handle.request(request).await,
        }
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        let method = request.method_name();
        let response = self.request(request).await;
        decode_typed_response(method, response)
    }
}

impl AppServerClient {
    /// App-server platform family, which can differ from the executor's platform.
    /// Older remote servers may omit this metadata.
    pub fn platform_family(&self) -> Option<&str> {
        match self {
            Self::Remote(client) => client.platform_family(),
        }
    }

    /// App-server operating system as reported at initialization, including unknown values.
    pub fn platform_os(&self) -> Option<&str> {
        match self {
            Self::Remote(client) => client.platform_os(),
        }
    }

    pub fn codex_home(&self, _local_codex_home: &AbsolutePathBuf) -> Option<AppServerPath> {
        match self {
            Self::Remote(client) => client.codex_home().map(AppServerPath::from_app_server),
        }
    }

    pub async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        match self {
            Self::Remote(client) => client.request(request).await,
        }
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        let method = request.method_name();
        let response = self.request(request).await;
        decode_typed_response(method, response)
    }

    pub async fn notify(&self, notification: ClientNotification) -> IoResult<()> {
        match self {
            Self::Remote(client) => client.notify(notification).await,
        }
    }

    pub async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> IoResult<()> {
        match self {
            Self::Remote(client) => client.resolve_server_request(request_id, result).await,
        }
    }

    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> IoResult<()> {
        match self {
            Self::Remote(client) => client.reject_server_request(request_id, error).await,
        }
    }

    pub async fn next_event(&mut self) -> Option<AppServerEvent> {
        match self {
            Self::Remote(client) => client.next_event().await,
        }
    }

    pub async fn shutdown(self) -> IoResult<()> {
        match self {
            Self::Remote(client) => client.shutdown().await,
        }
    }

    pub fn request_handle(&self) -> AppServerRequestHandle {
        match self {
            Self::Remote(client) => AppServerRequestHandle::Remote(client.request_handle()),
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test]
    async fn websocket_routes_rpc_notifications_and_server_approvals() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let initialize: serde_json::Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(initialize["method"], "initialize");
            socket.send(Message::Text(json!({"id":initialize["id"],"result":{"platformFamily":"unix","platformOs":"linux"}}).to_string().into())).await.unwrap();
            let initialized: serde_json::Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(initialized["method"], "initialized");
            let request: serde_json::Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(request["method"], "model/list");
            for response in [
                json!({"id":request["id"],"result":{"data":[],"nextCursor":null}}),
                json!({"method":"thread/name/updated","params":{"threadId":"thread","threadName":"Remote"}}),
                json!({"id":"approval","method":"item/tool/requestUserInput","params":{"threadId":"thread","turnId":"turn","itemId":"item","questions":[]}}),
            ] {
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
            }
            let approval: serde_json::Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            assert_eq!(approval["id"], "approval");
            assert_eq!(approval["result"], json!({"answers":{}}));
            socket.close(None).await.unwrap();
        });
        let mut client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url: format!("ws://{address}/rpc"),
                auth_token: None,
            },
            client_name: "litter-test".into(),
            client_version: "1".into(),
            experimental_api: true,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: 16,
        })
        .await
        .unwrap();
        let request: ClientRequest =
            serde_json::from_value(json!({"id":1,"method":"model/list","params":{}})).unwrap();
        let result = client.request(request).await.unwrap().unwrap();
        assert_eq!(result["data"], json!([]));
        let mut notified = false;
        let mut approved = false;
        tokio::time::timeout(Duration::from_secs(3), async {
            while !(notified && approved) {
                match client.next_event().await.unwrap() {
                    AppServerEvent::ServerNotification(notification) => {
                        assert!(matches!(
                            *notification,
                            ServerNotification::ThreadNameUpdated(_)
                        ));
                        notified = true;
                    }
                    AppServerEvent::ServerRequest(request) => {
                        let id = serde_json::to_value(request).unwrap()["id"].clone();
                        client
                            .resolve_server_request(
                                serde_json::from_value(id).unwrap(),
                                json!({"answers":{}}),
                            )
                            .await
                            .unwrap();
                        approved = true;
                    }
                    event => panic!("unexpected remote event: {event:?}"),
                }
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        client.shutdown().await.unwrap();
    }
}
