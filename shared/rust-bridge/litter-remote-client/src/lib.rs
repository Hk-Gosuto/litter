//! Remote app-server client, adapted from OpenAI Codex rust-v0.162.0-alpha.4.
//! Apache-2.0; see LICENSE. Embedded app-server startup is deliberately excluded.
mod path;
mod remote;
use std::error::Error;
use std::fmt;
use std::io::{Error as IoError, Result as IoResult};
use std::time::Duration;
use codex_app_server_protocol::{ClientNotification, ClientRequest, JSONRPCErrorError, RequestId, Result as JsonRpcResult, ServerNotification, ServerRequest};
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::de::DeserializeOwned;
pub use path::AppServerPath;
pub use remote::{JsonRpcWire, RemoteAppServerClient, RemoteAppServerConnectArgs, RemoteAppServerEndpoint};
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
pub type RequestResult = std::result::Result<JsonRpcResult, JSONRPCErrorError>;

#[derive(Debug, Clone)]
pub enum AppServerEvent {
    Lagged { skipped: usize },
    ServerNotification(Box<ServerNotification>),
    ServerRequest(Box<ServerRequest>),
    Disconnected { message: String },
}

/// Layered error for [`InProcessAppServerClient::request_typed`].
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
pub enum AppServerRequestHandle { Remote(remote::RemoteAppServerRequestHandle) }
pub enum AppServerClient { Remote(RemoteAppServerClient) }
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

