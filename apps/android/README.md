# Android App

Android runtime is now on the same Rust-first architecture as iOS:

- `app`: Android entrypoint/activity.
- `core:bridge`: UniFFI-generated bindings plus Android Rust init/bootstrap.

## Runtime Architecture

- Canonical runtime state lives in Rust `AppStore` and is observed from `app/src/main/java/com/litter/android/state/AppModel.kt`.
- Direct server operations come from the shared Rust `AppClient` surface.
- Add Server uses explicit kittylitter, Local Studio, connected-computer, and
  manual SSH/Codex URL paths. The current Android chooser does not run a
  background NSD or subnet-discovery scan.
- SSH uses Rust `SshBridge`.
- Voice runtime uses Rust store/RPC for realtime state and Android-only code for audio capture/playback, AEC, and services.

## Remote-only Runtime

- Android does not start an on-device Codex process. Every session runs on a
  connected computer or remote host through WebSocket, SSH, Alleycat, or
  Slingshot transport.
- `codex-mobile-client` is the single shared Rust client surface for both iOS
  and Android; it also owns the Android JNI bootstrap (`nativeBridgeInit` +
  `nativeMobileClientInit` in `android_context.rs`).
- Legacy local bootstrap symbols remain in the generated API for migration
  compatibility and return `Unsupported` when called.

Examples:

```bash
./gradlew :app:assembleDebug
```

Open in Android Studio (macOS):

```bash
open -a "Android Studio" apps/android
```

Rebuild + reopen workflow:

```bash
./apps/android/scripts/rebuild-and-reopen.sh
```

Optional controls:

```bash
./apps/android/scripts/rebuild-and-reopen.sh --with-rust
./apps/android/scripts/rebuild-and-reopen.sh --no-open
```

QA matrix and regression command list: `apps/android/docs/qa-matrix.md`.

## Rust Bridge (Android)

Android loads the Rust shared library `libcodex_mobile_client.so` through UniFFI init in `core:bridge`.
The generated Kotlin bindings live under `shared/rust-bridge/generated/kotlin/` and are consumed directly by `apps/android/core/bridge`.

Build and copy JNI artifacts into `core:bridge`:

```bash
./tools/scripts/build-android-rust.sh
```

Prerequisites:

- Android NDK (`ANDROID_NDK_HOME` or `ANDROID_NDK_ROOT` set)
- `cargo-ndk` (`cargo install cargo-ndk`)
