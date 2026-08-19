use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use crate::mobile_exec_command::mobile_system_command;

static ISH_EXEC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();

pub(crate) fn install() {
    ISH_EXEC_HOOK_INSTALLED.get_or_init(|| {
        codex_core::exec::set_ios_exec_hook(run_command);
        codex_core::exec::set_ios_streaming_exec_hook(run_command_streaming);
        codex_core::exec::set_ios_live_exec_spawn_hook(crate::ish_runtime::spawn_live);
        crate::shell_preflight::install();
    });
}

pub(crate) fn run_command(
    argv: &[String],
    cwd: &Path,
    _env: &HashMap<String, String>,
    timeout_ms: Option<u64>,
    cancel: &codex_core::exec::IosExecCancel,
) -> (i32, Vec<u8>) {
    if let Some(result) = run_apply_patch_if_requested(argv, cwd, timeout_ms, cancel) {
        return result;
    }

    let cmd = mobile_system_command(argv);
    eprintln!("[ish-exec] run: {cmd} (cwd={})", cwd.display());

    let cwd_str = cwd.to_string_lossy();
    let (code, output) = crate::ish_runtime::run_streaming(
        &cmd,
        Some(cwd_str.as_ref()),
        timeout_ms,
        Some(cancel),
        |_| {},
    );

    let preview = String::from_utf8_lossy(&output);
    let preview = if preview.len() > 200 {
        &preview[..200]
    } else {
        &preview
    };
    eprintln!(
        "[ish-exec] exit={code} output_len={} preview={preview}",
        output.len()
    );

    (code, output)
}

pub(crate) fn run_command_streaming(
    argv: &[String],
    cwd: &Path,
    _env: &HashMap<String, String>,
    timeout_ms: Option<u64>,
    on_output: codex_core::exec::IosExecOutputHandler,
    cancel: &codex_core::exec::IosExecCancel,
) -> (i32, Vec<u8>) {
    if let Some((code, output)) = run_apply_patch_if_requested(argv, cwd, timeout_ms, cancel) {
        if !output.is_empty() {
            on_output(output.clone());
        }
        return (code, output);
    }

    let cmd = mobile_system_command(argv);
    eprintln!("[ish-exec] run(streaming): {cmd} (cwd={})", cwd.display());

    let cwd_str = cwd.to_string_lossy();
    let (code, output) = crate::ish_runtime::run_streaming(
        &cmd,
        Some(cwd_str.as_ref()),
        timeout_ms,
        Some(cancel),
        |chunk| {
            if !chunk.is_empty() {
                on_output(chunk.to_vec());
            }
        },
    );

    let preview = String::from_utf8_lossy(&output);
    let preview = if preview.len() > 200 {
        &preview[..200]
    } else {
        &preview
    };
    eprintln!(
        "[ish-exec] exit(streaming)={code} output_len={} preview={preview}",
        output.len()
    );

    (code, output)
}

fn run_apply_patch_if_requested(
    argv: &[String],
    cwd: &Path,
    timeout_ms: Option<u64>,
    cancel: &codex_core::exec::IosExecCancel,
) -> Option<(i32, Vec<u8>)> {
    if !argv
        .iter()
        .any(|arg| arg == codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1)
    {
        return None;
    }
    let patch = argv
        .iter()
        .skip_while(|arg| *arg != codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1)
        .nth(1)?;

    eprintln!("[ish-exec] apply_patch in-process (cwd={})", cwd.display());
    if cancel.is_cancelled() {
        return Some((1, b"command cancelled\n".to_vec()));
    }
    let cwd_abs = match codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(cwd) {
        Ok(abs) => abs,
        Err(err) => {
            let msg = format!("invalid cwd for apply_patch: {err}\n");
            eprintln!("[ish-exec] apply_patch setup error: {err}");
            return Some((1, msg.into_bytes()));
        }
    };
    let cwd_uri = codex_utils_path_uri::PathUri::from_abs_path(&cwd_abs);
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let fs = codex_exec_server::LOCAL_FS.clone();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let msg = format!("build tokio runtime for apply_patch: {err}\n");
            eprintln!("[ish-exec] apply_patch runtime error: {err}");
            return Some((1, msg.into_bytes()));
        }
    };
    let outcome = runtime.block_on(async {
        let apply = codex_apply_patch::apply_patch(
            patch,
            &cwd_uri,
            &mut stdout_buf,
            &mut stderr_buf,
            fs.as_ref(),
            None,
        );
        tokio::select! {
            biased;
            _ = cancel.cancelled() => ApplyPatchOutcome::Cancelled,
            _ = sleep_optional(timeout_ms) => ApplyPatchOutcome::TimedOut,
            result = apply => ApplyPatchOutcome::Finished(result),
        }
    });
    Some(match outcome {
        ApplyPatchOutcome::Cancelled => {
            let mut output = stdout_buf;
            output.extend_from_slice(&stderr_buf);
            output.extend_from_slice(b"command cancelled\n");
            (1, output)
        }
        ApplyPatchOutcome::TimedOut => {
            let mut output = stdout_buf;
            output.extend_from_slice(&stderr_buf);
            output.extend_from_slice(b"command timed out\n");
            (crate::ish_runtime::ISH_E_TIMEOUT, output)
        }
        ApplyPatchOutcome::Finished(result) => {
            let code = match result {
                Ok(_) => 0,
                Err(err) => {
                    eprintln!("[ish-exec] apply_patch error: {err}");
                    if stderr_buf.is_empty() {
                        stderr_buf = format!("{err}\n").into_bytes();
                    }
                    1
                }
            };
            let mut output = stdout_buf;
            output.extend_from_slice(&stderr_buf);
            eprintln!(
                "[ish-exec] apply_patch exit={code} output_len={}",
                output.len()
            );
            (code, output)
        }
    })
}

enum ApplyPatchOutcome {
    Cancelled,
    TimedOut,
    Finished(Result<codex_apply_patch::AppliedPatchDelta, codex_apply_patch::ApplyPatchFailure>),
}

async fn sleep_optional(timeout_ms: Option<u64>) {
    match timeout_ms {
        Some(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
        None => std::future::pending::<()>().await,
    }
}
