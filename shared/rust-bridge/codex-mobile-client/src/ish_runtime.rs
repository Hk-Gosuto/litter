//! Stubbed local shell surface for the remote-only mobile build.
//!
//! Remote Codex sessions execute on the paired server over app-server RPC or
//! SSH. The embedded iSH runtime is intentionally absent from this variant;
//! these compatibility functions keep the existing UniFFI surface stable
//! while reporting that local execution is unavailable.

use std::collections::HashMap;
use std::path::Path;

use crate::ish_types::IshBootstrapError;

pub const ISH_E_NOT_RUNNING: i32 = -6;
pub const ISH_E_TIMEOUT: i32 = -8;

pub async fn instance_or_wait(_timeout: std::time::Duration) -> Option<()> {
    None
}

pub fn bootstrap(
    _bundle_fs_path: &Path,
    _application_support_dir: &Path,
    _documents_dir: &Path,
) -> Result<(), IshBootstrapError> {
    Err(IshBootstrapError::Unsupported {
        detail: "embedded iSH is disabled in the remote-only build".to_string(),
    })
}

pub fn default_cwd() -> &'static str {
    "/root"
}

pub fn run(_cmd: &str, _cwd: Option<&str>, _timeout_ms: Option<u64>) -> (i32, Vec<u8>) {
    (
        ISH_E_NOT_RUNNING,
        b"embedded iSH is disabled; use a remote Codex session\n".to_vec(),
    )
}

pub fn run_streaming<F>(
    _cmd: &str,
    _cwd: Option<&str>,
    _timeout_ms: Option<u64>,
    mut on_output: F,
) -> (i32, Vec<u8>)
where
    F: FnMut(&[u8]),
{
    let output = b"embedded iSH is disabled; use a remote Codex session\n".to_vec();
    on_output(&output);
    (ISH_E_NOT_RUNNING, output)
}

pub fn runtime_env() -> HashMap<String, String> {
    HashMap::new()
}
