//! Remote-only compatibility surface for former local process bindings.
use std::path::{Path, PathBuf};
pub(crate) const fn shell_candidate_lines() -> &'static [&'static str] {
    alleycat_bridge_core::codex_resolver::POSIX_SHELL_CANDIDATE_LINES
}
pub async fn probe_local_server(_port: u16) -> bool {
    false
}
pub fn resolve_codex_binary_local() -> Option<PathBuf> {
    None
}
#[derive(Debug, thiserror::Error)]
#[error("Local Codex is unavailable. Connect a remote computer.")]
pub struct LocalServerError;
pub struct LocalServerHandle;
impl LocalServerHandle {
    pub async fn stop(self) {}
    pub fn port(&self) -> u16 {
        0
    }
    pub fn codex_path(&self) -> &Path {
        Path::new("")
    }
}
pub struct LocalServerAttach {
    pub port: u16,
    pub handle: Option<LocalServerHandle>,
    pub attached_to_existing: bool,
    pub codex_path: Option<PathBuf>,
}
pub async fn attach_or_spawn_local_server(
    _port: u16,
    _codex_home: Option<PathBuf>,
) -> Result<LocalServerAttach, LocalServerError> {
    Err(LocalServerError)
}
