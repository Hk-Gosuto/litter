//! iOS-only iSH bootstrap + run surface. Port of the former Obj-C
//! `apps/ios/Sources/Litter/Bridge/IshBridge.{h,m}` into Rust.
//!
//! Responsibilities, mirroring the Obj-C original 1:1:
//! 1. Extract the bundled `fs` rootfs into `<app_support>/fs/` on first launch.
//! 2. `chmod 0644` the fakefs `meta.db` so SQLite can write.
//! 3. Boot the iSH kernel at `<app_support>/fs/data` with `/root` as cwd.
//! 4. Install small runtime directories and pass the iSH environment on every
//!    command (`LANG`, `TMPDIR`, `CODEX_HOME`, …).
//! 5. Snapshot host DNS into `/etc/resolv.conf` inside the fakefs.
//! 6. Mount `<documents>/Apps/` at `/mnt/apps/` via iSH's `realfs` driver.
//! 7. Register the `codex_core` exec hook (`ish_exec::install()`).
//!
//! After `bootstrap`, `run(cmd, cwd)` dispatches command strings through the
//! persistent `/bin/sh` the same way `codex_ish_run` did in Obj-C.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_uint};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ish_embed_host::IshInstance;
use ish_embed_host::PtySize;
use ish_embed_host::SpawnOpts;
use ish_embed_host::WriteStatus;

use crate::ish_types::IshBootstrapError;

// Numeric error codes preserved for back-compat with the previous `ish` crate
// surface. The Swift side observes these as negative `Int32` values.
pub const ISH_E_BOOT: i32 = -1;
pub const ISH_E_MOUNT: i32 = -2;
pub const ISH_E_EXECVE: i32 = -3;
pub const ISH_E_PIPE: i32 = -4;
pub const ISH_E_THREAD: i32 = -5;
pub const ISH_E_NOT_RUNNING: i32 = -6;
pub const ISH_E_IO: i32 = -7;
pub const ISH_E_TIMEOUT: i32 = -8;
pub const ISH_E_NOMEM: i32 = -9;
pub const ISH_E_ARGS: i32 = -10;
const BOOTSTRAP_COMMAND_TIMEOUT_MS: u64 = 10_000;
const ROOTFS_STAMP_FILE: &str = ".litter-rootfs-id";
const ROOTFS_ARCH_FILE: &str = "data/etc/apk/arch";
const ROOTFS_ALPINE_RELEASE_FILE: &str = "data/etc/alpine-release";
const ROOTFS_ROOT_HOME_DIR: &str = "data/root";

impl From<ish_embed_host::IshError> for IshBootstrapError {
    fn from(err: ish_embed_host::IshError) -> Self {
        IshBootstrapError::Ish(err.to_string())
    }
}

static INSTANCE: OnceLock<IshInstance> = OnceLock::new();

/// Wait up to `timeout` for `bootstrap` to finish on another thread. Returns
/// the live instance once it's published, or `None` if the timeout elapses.
/// Used by the terminal session opener so a UI tap that races the on-launch
/// bootstrap doesn't surface a misleading "iSH has not been bootstrapped"
/// error to the user.
pub(crate) async fn instance_or_wait(timeout: std::time::Duration) -> Option<&'static IshInstance> {
    if let Some(instance) = INSTANCE.get() {
        return Some(instance);
    }
    let deadline = std::time::Instant::now() + timeout;
    let poll = std::time::Duration::from_millis(100);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(poll).await;
        if let Some(instance) = INSTANCE.get() {
            return Some(instance);
        }
    }
    None
}

/// One-time iSH boot. Mirrors `codex_ish_init` + the post-init setup calls in
/// IshBridge.m. After this returns `Ok`, `run()` is safe to call and the
/// codex_core exec hook has been installed.
///
/// * `bundle_fs_path` — absolute path to the `fs` directory inside the app
///   bundle (Swift resolves this via `Bundle.main.url(forResource:"fs", …)`).
/// * `application_support_dir` — Application Support dir for the app; the
///   rootfs lives under `<application_support_dir>/fs/`.
/// * `documents_dir` — the app's Documents directory; `Apps/` inside it is
///   bind-mounted at `/mnt/apps` inside the fakefs.
pub fn bootstrap(
    bundle_fs_path: &Path,
    application_support_dir: &Path,
    documents_dir: &Path,
) -> Result<(), IshBootstrapError> {
    if INSTANCE.get().is_some() {
        return Err(IshBootstrapError::AlreadyBootstrapped);
    }

    let dest = application_support_dir.join("fs");
    extract_rootfs_if_needed(bundle_fs_path, &dest)?;

    let meta_db = dest.join("meta.db");
    if meta_db.exists() {
        let mut perms = fs::metadata(&meta_db)?.permissions();
        perms.set_mode(0o644);
        if let Err(err) = fs::set_permissions(&meta_db, perms) {
            eprintln!("[ish] chmod 0644 on meta.db failed: {err}");
        }
    }

    let data_path = dest.join("data");
    eprintln!(
        "[ish] booting kernel with rootfs='{}' workdir='/root'",
        data_path.display()
    );
    let instance = IshInstance::boot(&data_path, Some(Path::new("/root")))?;
    eprintln!("[ish] kernel booted");

    INSTANCE
        .set(instance)
        .map_err(|_| IshBootstrapError::AlreadyBootstrapped)?;

    // Now that INSTANCE is published, the post-init setup goes through the
    // normal run() path, which takes the shared lock and honors the same
    // ordering guarantees as regular command dispatch.
    runtime_setup();
    write_resolv_conf();
    mount_apps_dir(documents_dir);

    crate::ish_exec::install();

    Ok(())
}

/// Default working directory for iSH-backed local sessions. Port of
/// `codex_ish_default_cwd` — always `/root` (Alpine's root home).
pub fn default_cwd() -> &'static str {
    "/root"
}

/// Run `cmd` through the persistent `/bin/sh`. When `cwd` is non-empty the
/// command is wrapped as `cd '<cwd>' && <cmd>` (same shell-quote pass as the
/// Obj-C port). Returns (exit_code, merged stdout+stderr bytes). If the kernel
/// has not been booted or the FFI call fails, returns a negative ISH_E_* code
/// and an empty byte vector — matching the IshBridge.m error semantics so the
/// exec-hook path can surface the failure without a nil pointer panic.
pub fn run(cmd: &str, cwd: Option<&str>, timeout_ms: Option<u64>) -> (i32, Vec<u8>) {
    run_streaming(cmd, cwd, timeout_ms, None, |_| {})
}

pub fn run_streaming<F>(
    cmd: &str,
    cwd: Option<&str>,
    timeout_ms: Option<u64>,
    cancel: Option<&codex_core::exec::IosExecCancel>,
    mut on_output: F,
) -> (i32, Vec<u8>)
where
    F: FnMut(&[u8]),
{
    let Some(instance) = INSTANCE.get() else {
        eprintln!("[ish] run() called before bootstrap succeeded");
        return (ISH_E_NOT_RUNNING, Vec::new());
    };

    // The previous embed library funnelled every command through a single
    // persistent `/bin/sh`, so `cd` between calls leaked. The new architecture
    // forks a fresh shell per call, so an explicit `cd && cmd` wrapper is
    // still the right way to honour caller-supplied cwd.
    let wrapped = match cwd {
        Some(c) if !c.is_empty() => format!("cd {} && {}", shell_quote(c), cmd),
        _ => cmd.to_string(),
    };

    let argv = vec!["/bin/sh".to_string(), "-c".to_string(), wrapped];
    let env = runtime_env();
    let cwd_path = PathBuf::from("/");
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            return (ISH_E_THREAD, format!("runtime: {err}\n").into_bytes());
        }
    };
    rt.block_on(run_oneshot_cancellable(
        instance,
        argv,
        cwd_path,
        env,
        timeout_ms,
        cancel,
        &mut on_output,
    ))
}

fn terminate_session_offthread(session: Arc<ish_embed_host::IshSession>) {
    let _ = std::thread::Builder::new()
        .name("ish-interrupt".into())
        .spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                let _ = rt.block_on(session.terminate());
            }
        });
}

pub fn spawn_live(
    command: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    tty: bool,
) -> std::result::Result<Arc<dyn codex_core::exec::IosLiveExecProcess>, String> {
    let instance = INSTANCE
        .get()
        .ok_or_else(|| "iSH has not been bootstrapped".to_string())?;
    let cmd = crate::mobile_exec_command::mobile_system_command(command);
    let wrapped = match cwd.to_str() {
        Some(c) if !c.is_empty() => format!("cd {} && {}", shell_quote(c), cmd),
        _ => cmd,
    };
    let mut session_env = runtime_env();
    session_env.extend(env.iter().map(|(key, value)| (key.clone(), value.clone())));
    let opts = SpawnOpts {
        argv: vec!["/bin/sh".to_string(), "-c".to_string(), wrapped],
        envp: Some(session_env.into_iter().collect()),
        cwd: Some(PathBuf::from("/")),
        tty,
        size: tty.then_some(PtySize { cols: 80, rows: 24 }),
        pipe_stdin: !tty,
        arg0: None,
    };
    let session = instance
        .spawn(opts)
        .map_err(|err| format!("spawn live iSH process: {err}"))?;
    Ok(Arc::new(IshLiveProcess { session }))
}

struct IshLiveProcess {
    session: Arc<ish_embed_host::IshSession>,
}

impl codex_core::exec::IosLiveExecProcess for IshLiveProcess {
    fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Pin<
        Box<
            dyn Future<Output = std::result::Result<codex_core::exec::IosLiveRead, String>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let read = self
                .session
                .read(after_seq, max_bytes, wait_ms)
                .await
                .map_err(|err| err.to_string())?;
            Ok(codex_core::exec::IosLiveRead {
                chunks: read
                    .chunks
                    .into_iter()
                    .map(|chunk| (chunk.seq, chunk.bytes.to_vec()))
                    .collect(),
                next_seq: read.next_seq,
                exited: read.exited,
                exit_code: read.exit_code,
                closed: read.closed,
                failure: None,
            })
        })
    }

    fn write(
        &self,
        data: Vec<u8>,
    ) -> Pin<
        Box<
            dyn Future<Output = std::result::Result<codex_core::exec::IosLiveWriteStatus, String>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            match self.session.write(&data).await {
                Ok(WriteStatus::Accepted) => Ok(codex_core::exec::IosLiveWriteStatus::Accepted),
                Ok(WriteStatus::Starting) => Ok(codex_core::exec::IosLiveWriteStatus::Starting),
                Ok(WriteStatus::StdinClosed) => {
                    Ok(codex_core::exec::IosLiveWriteStatus::StdinClosed)
                }
                Ok(WriteStatus::UnknownSession) => {
                    Ok(codex_core::exec::IosLiveWriteStatus::UnknownSession)
                }
                Err(err) => Err(err.to_string()),
            }
        })
    }

    fn signal(
        &self,
        signum: i32,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + '_>> {
        Box::pin(async move { self.session.signal(signum).await.map_err(|err| err.to_string()) })
    }

    fn terminate(&self) -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + '_>> {
        Box::pin(async move { self.session.terminate().await.map_err(|err| err.to_string()) })
    }
}

async fn run_oneshot_cancellable<F>(
    instance: &IshInstance,
    argv: Vec<String>,
    cwd: PathBuf,
    env: HashMap<String, String>,
    timeout_ms: Option<u64>,
    cancel: Option<&codex_core::exec::IosExecCancel>,
    on_output: &mut F,
) -> (i32, Vec<u8>)
where
    F: FnMut(&[u8]),
{
    if cancel.is_some_and(codex_core::exec::IosExecCancel::is_cancelled) {
        return (1, b"command cancelled\n".to_vec());
    }
    let opts = SpawnOpts {
        argv,
        envp: Some(env.into_iter().collect()),
        cwd: Some(cwd),
        tty: false,
        size: None,
        pipe_stdin: false,
        arg0: None,
    };
    let session = match instance.spawn(opts) {
        Ok(session) => session,
        Err(err) => return (ISH_E_EXECVE, format!("spawn: {err}\n").into_bytes()),
    };
    if let Some(cancel) = cancel {
        let interrupt = Arc::clone(&session);
        cancel.set_on_cancel(move || terminate_session_offthread(Arc::clone(&interrupt)));
        if cancel.is_cancelled() {
            cancel.clear_on_cancel();
            let _ = session.terminate().await;
            return (1, b"command cancelled\n".to_vec());
        }
    }
    let result = drain_oneshot(&session, timeout_ms, on_output, cancel).await;
    if let Some(cancel) = cancel {
        cancel.clear_on_cancel();
    }
    result
}

async fn drain_oneshot<F>(
    session: &ish_embed_host::IshSession,
    timeout_ms: Option<u64>,
    on_output: &mut F,
    cancel: Option<&codex_core::exec::IosExecCancel>,
) -> (i32, Vec<u8>)
where
    F: FnMut(&[u8]),
{
    use tokio::time::Instant;

    let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    let mut output = Vec::new();
    let mut next_seq: u64 = 0;
    loop {
        if cancel.is_some_and(codex_core::exec::IosExecCancel::is_cancelled) {
            let _ = session.terminate().await;
            return (1, output);
        }
        let remaining = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        if remaining == Some(Duration::ZERO) {
            let _ = session.terminate().await;
            return (ISH_E_TIMEOUT, output);
        }
        let wait_ms = remaining.map(|duration| duration.as_millis() as u64).unwrap_or(60_000);
        let read = match session
            .read(Some(next_seq), Some(64 * 1024), Some(wait_ms))
            .await
        {
            Ok(read) => read,
            Err(err) => {
                output.extend_from_slice(format!("read error: {err}\n").as_bytes());
                return (ISH_E_IO, output);
            }
        };
        for chunk in &read.chunks {
            output.extend_from_slice(&chunk.bytes);
            on_output(&chunk.bytes);
            next_seq = chunk.seq;
        }
        if read.closed {
            return (read.exit_code.unwrap_or(-1), output);
        }
    }
}

// ── post-init setup helpers ──────────────────────────────────────────────
// These mirror codex_ish_runtime_setup / codex_ish_write_resolv_conf /
// codex_ish_mount_apps_dir from IshBridge.m. They call run() internally; the
// ish crate's own lock serializes the actual dispatches.

const RUNTIME_SETUP_SCRIPT: &str = concat!(
    "mkdir -p /root/.codex /tmp ;",
    "chmod 700 /root/.codex ;",
    "chmod 1777 /tmp",
);

pub(crate) fn runtime_env() -> HashMap<String, String> {
    HashMap::from([
        (
            "PATH".to_string(),
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        ),
        ("HOME".to_string(), "/root".to_string()),
        ("USER".to_string(), "root".to_string()),
        ("LOGNAME".to_string(), "root".to_string()),
        ("LANG".to_string(), "C.UTF-8".to_string()),
        ("LC_ALL".to_string(), "C.UTF-8".to_string()),
        ("TMPDIR".to_string(), "/tmp".to_string()),
        // No tty under the exec hook: force pagers to dump and exit so
        // commands like `git log` do not block waiting for interaction.
        ("PAGER".to_string(), "cat".to_string()),
        ("EDITOR".to_string(), "vi".to_string()),
        ("HOSTNAME".to_string(), "litter".to_string()),
        // Symmetric with the native CODEX_HOME used by the Rust process.
        // Tools inside iSH need a fakefs-local config path.
        ("CODEX_HOME".to_string(), "/root/.codex".to_string()),
    ])
}

fn runtime_setup() {
    let (rc, _) = run(
        RUNTIME_SETUP_SCRIPT,
        None,
        Some(BOOTSTRAP_COMMAND_TIMEOUT_MS),
    );
    if rc != 0 {
        eprintln!("[ish] runtime setup failed rc={rc}");
    }
}

fn write_resolv_conf() {
    let body = resolv_conf_body();
    let cmd = format!("printf %s {} > /etc/resolv.conf", shell_quote(&body));
    let (rc, _) = run(&cmd, None, Some(BOOTSTRAP_COMMAND_TIMEOUT_MS));
    if rc != 0 {
        eprintln!("[ish] failed to write /etc/resolv.conf rc={rc}");
    } else {
        eprintln!("[ish] /etc/resolv.conf installed ({} bytes)", body.len());
    }
}

fn mount_apps_dir(documents_dir: &Path) {
    let apps_dir = documents_dir.join("Apps");
    if let Err(err) = fs::create_dir_all(&apps_dir) {
        eprintln!("[ish] could not create {}: {err}", apps_dir.display());
        return;
    }
    let Some(apps_str) = apps_dir.to_str() else {
        eprintln!("[ish] apps dir not utf-8: {}", apps_dir.display());
        return;
    };
    let cmd = format!(
        "mkdir -p /mnt/apps && mount -t real {} /mnt/apps",
        shell_quote(apps_str)
    );
    let (rc, _) = run(&cmd, None, Some(BOOTSTRAP_COMMAND_TIMEOUT_MS));
    if rc != 0 {
        eprintln!("[ish] mount /mnt/apps failed rc={rc}");
    } else {
        eprintln!("[ish] /mnt/apps mounted from '{}'", apps_str);
    }
}

// ── bundled rootfs extraction ────────────────────────────────────────────

fn extract_rootfs_if_needed(source: &Path, dest: &Path) -> Result<(), IshBootstrapError> {
    if !source.is_dir() {
        return Err(IshBootstrapError::BundledRootfsMissing(
            source.display().to_string(),
        ));
    }

    if dest.is_dir() {
        let source_identity = rootfs_identity(source)?;
        let dest_identity = rootfs_identity(dest)?;
        if rootfs_identity_matches(source_identity.as_ref(), dest_identity.as_ref()) {
            return Ok(());
        }
        eprintln!(
            "[ish] rootfs identity changed bundled={} installed={}",
            display_rootfs_identity(source_identity.as_ref()),
            display_rootfs_identity(dest_identity.as_ref())
        );
    }

    if dest.exists() {
        eprintln!(
            "[ish] replacing extracted rootfs at '{}' with bundled rootfs",
            dest.display()
        );
    }
    replace_dir_recursive(source, dest)?;
    Ok(())
}

fn rootfs_identity(root: &Path) -> io::Result<Option<String>> {
    if let Some(stamp) = read_rootfs_stamp(root)? {
        return Ok(Some(format!("stamp:{stamp}")));
    }

    let arch = read_trimmed_file(root.join(ROOTFS_ARCH_FILE))?;
    let alpine_release = read_trimmed_file(root.join(ROOTFS_ALPINE_RELEASE_FILE))?;
    if let Some(arch) = arch.or_else(|| detect_musl_arch(root)) {
        return Ok(Some(format!(
            "arch:{arch};alpine:{}",
            alpine_release.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(None)
}

fn rootfs_identity_matches(source: Option<&String>, dest: Option<&String>) -> bool {
    match source {
        Some(source) => dest.is_some_and(|dest| dest == source),
        None => true,
    }
}

fn display_rootfs_identity(identity: Option<&String>) -> &str {
    identity.map(String::as_str).unwrap_or("unknown")
}

fn read_rootfs_stamp(root: &Path) -> io::Result<Option<String>> {
    read_trimmed_file(root.join(ROOTFS_STAMP_FILE))
}

fn read_trimmed_file(path: impl AsRef<Path>) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(value) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value.to_string()))
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn detect_musl_arch(root: &Path) -> Option<String> {
    let entries = fs::read_dir(root.join("data/lib")).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.contains("musl-aarch64") {
            return Some("aarch64".to_string());
        }
        if name.contains("musl-x86") || name.contains("musl-i386") {
            return Some("x86".to_string());
        }
    }
    None
}

fn replace_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    let name = dst
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("fs");
    let tmp = dst.with_file_name(format!(".{name}.tmp-{}", std::process::id()));

    remove_path_if_exists(&tmp)?;
    copy_dir_recursive(src, &tmp)?;
    preserve_root_home(dst, &tmp)?;
    remove_path_if_exists(dst)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

fn preserve_root_home(old_root: &Path, new_root: &Path) -> io::Result<()> {
    let old_home = old_root.join(ROOTFS_ROOT_HOME_DIR);
    let old_meta = match fs::symlink_metadata(&old_home) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if !old_meta.is_dir() {
        return Ok(());
    }

    let new_home = new_root.join(ROOTFS_ROOT_HOME_DIR);
    remove_path_if_exists(&new_home)?;
    if let Some(parent) = new_home.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_dir_recursive(&old_home, &new_home)?;
    eprintln!("[ish] preserved /root across rootfs replacement");
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ft.is_symlink() {
            let target = fs::read_link(&src_path)?;
            // Unix symlink copy; iSH fakefs data dir contains plain files +
            // dirs in practice, but tolerate symlinks in the bundle just in
            // case.
            std::os::unix::fs::symlink(&target, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

use crate::shell_quoting::posix_quote as shell_quote;

// ── resolv.conf snapshot (libresolv FFI) ─────────────────────────────────
//
// Apple's <resolv.h> macro-renames `res_ninit` / `res_getservers` /
// `res_ndestroy` to `res_9_*`, so libresolv.9.tbd on iPhoneOS.sdk exports
// the `res_9_*` symbols. The Rust FFI declares those names directly.
//
// We reproduce `codex_ish_resolv_conf_body()` from IshBridge.m with one
// intentional scope narrowing: we do not emit the `search …` line. Reading
// `struct __res_state::dnsrch` requires reaching into an opaque Apple
// resolver struct with no stable ABI contract; nameservers alone are enough
// for the bootstrap script to reach apk/curl, which is what the original
// Obj-C path was protecting. Empty search list falls through to the public
// resolver fallback below, matching the Obj-C "empty ⇒ fallback" semantic.

// Size chosen generously: the 64-bit Apple `struct __res_state` layout is
// around 1 KB (see resolv.h:182-232; includes MAXNS=3 sockaddr_in slots,
// MAXDNSRCH+1=7 char* pointers, and a 72-byte `_u` union). 4 KB zeroed is a
// safe upper bound that doesn't depend on Apple's internal offsets staying
// stable across SDKs.
const RES_STATE_BUF: usize = 4096;
// `union res_sockaddr_union` is 128-byte `__space` plus alignment padding
// (resolv.h:242-253). 256 bytes is the safe upper bound.
const RES_SOCKADDR_UNION_BUF: usize = 256;
// `<arpa/nameser.h>` / `<resolv.h>` — maximum name servers res_getservers
// will return.
const MAXNS: c_int = 3;

// `<netdb.h>` on Apple.
const NI_MAXHOST: usize = 1025;
const NI_NUMERICHOST: c_int = 0x0000_0002;

#[repr(C)]
struct Sockaddr {
    sa_len: u8,
    sa_family: u8,
    _opaque: [u8; 254],
}

unsafe extern "C" {
    fn res_9_ninit(state: *mut u8) -> c_int;
    fn res_9_getservers(state: *mut u8, servers: *mut u8, count: c_int) -> c_int;
    fn res_9_ndestroy(state: *mut u8);

    fn getnameinfo(
        sa: *const Sockaddr,
        salen: c_uint,
        host: *mut c_char,
        hostlen: c_uint,
        serv: *mut c_char,
        servlen: c_uint,
        flags: c_int,
    ) -> c_int;
}

fn resolv_conf_body() -> String {
    let mut out = String::new();

    let mut res_state = [0u8; RES_STATE_BUF];
    // SAFETY: res_state is a zeroed byte buffer sized generously above the
    // Apple `__res_state` struct. res_9_ninit writes through the pointer; we
    // never dereference fields on the Rust side. res_9_ndestroy is called
    // unconditionally on the Ok path, balancing res_9_ninit.
    let init_rc = unsafe { res_9_ninit(res_state.as_mut_ptr()) };
    if init_rc == 0 {
        let mut servers = [0u8; RES_SOCKADDR_UNION_BUF * MAXNS as usize];
        let found =
            unsafe { res_9_getservers(res_state.as_mut_ptr(), servers.as_mut_ptr(), MAXNS) };
        for i in 0..found.max(0) {
            // SAFETY: Each sockaddr_union slot is RES_SOCKADDR_UNION_BUF
            // bytes; the first byte is sin_len (Apple BSD sockaddr has
            // sa_len as the first byte). A zero sin_len means the slot was
            // left empty by the resolver — skip it, matching IshBridge.m.
            let slot = unsafe { servers.as_ptr().add(i as usize * RES_SOCKADDR_UNION_BUF) };
            let sa_len = unsafe { *slot };
            if sa_len == 0 {
                continue;
            }
            let mut host_buf = [0i8; NI_MAXHOST];
            let rc = unsafe {
                getnameinfo(
                    slot as *const Sockaddr,
                    sa_len as c_uint,
                    host_buf.as_mut_ptr(),
                    NI_MAXHOST as c_uint,
                    std::ptr::null_mut(),
                    0,
                    NI_NUMERICHOST,
                )
            };
            if rc == 0 {
                // SAFETY: getnameinfo NUL-terminates on success.
                let addr = unsafe { CStr::from_ptr(host_buf.as_ptr()) };
                if let Ok(s) = addr.to_str() {
                    out.push_str("nameserver ");
                    out.push_str(s);
                    out.push('\n');
                }
            }
        }
        unsafe { res_9_ndestroy(res_state.as_mut_ptr()) };
    }

    if !out.contains("nameserver ") {
        // Fallback: public resolvers so apk/curl still work when the host
        // resolver handed back nothing (offline, fresh container, etc.).
        out.push_str("nameserver 1.1.1.1\nnameserver 8.8.8.8\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_basic() {
        assert_eq!(shell_quote("x"), "'x'");
    }

    #[test]
    fn shell_quote_with_single_quote() {
        assert_eq!(shell_quote("x's"), "'x'\\''s'");
    }

    #[test]
    fn shell_quote_path_with_spaces() {
        assert_eq!(shell_quote("/var/Documents/Apps"), "'/var/Documents/Apps'");
    }
}
