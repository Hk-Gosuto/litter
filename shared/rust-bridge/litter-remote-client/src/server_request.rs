//! Connection-bound, single-use replies, adapted from upstream Codex #53049.

use crate::AppServerRequestHandle;
use codex_app_server_protocol::{JSONRPCErrorError, RequestId, Result as JsonRpcResult};
use std::fmt;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct AppServerReplyHandle {
    client: AppServerRequestHandle,
    request_id: RequestId,
    claimed: Arc<AtomicBool>,
}

impl fmt::Debug for AppServerReplyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppServerReplyHandle([redacted])")
    }
}

impl AppServerRequestHandle {
    /// Capture at ingress: an ID alone cannot identify a connection after reconnect.
    pub fn server_request_reply_handle(&self, request_id: RequestId) -> AppServerReplyHandle {
        AppServerReplyHandle {
            client: self.clone(),
            request_id,
            claimed: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AppServerReplyHandle {
    fn claim(&self) -> io::Result<()> {
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "app-server reply handle already used",
                )
            })
    }

    /// A failed send has an uncertain outcome and must not be retried on another connection.
    pub async fn resolve(&self, result: JsonRpcResult) -> io::Result<()> {
        self.claim()?;
        self.client
            .resolve_server_request(self.request_id.clone(), result)
            .await
    }

    pub async fn reject(&self, error: JSONRPCErrorError) -> io::Result<()> {
        self.claim()?;
        self.client
            .reject_server_request(self.request_id.clone(), error)
            .await
    }
}
