# Remote app-server client

Apache-2.0 transport adapted from OpenAI Codex `rust-v0.162.0-alpha.4`:
https://github.com/openai/codex/tree/fe3894843141854f0d8e4298ece89fdeedd089c4/codex-rs/app-server-client

`remote.rs` and `path.rs` retain the upstream remote handshake, request router,
notifications, approvals and shutdown behavior, plus Litter's generic stream
adapter for SSH and JSONL. `lib.rs` supplies only the remote facade; there is
no in-process startup or dependency on codex-core/app-server. Cargo aliases
this package as codex-app-server-client for existing internal imports. This
crate has no UniFFI exports; codex-mobile-client remains the sole mobile FFI.

On a transport upgrade, compare these files with the pinned upstream remote
implementation, retain the generic wire entry points, and run the loopback
WebSocket integration test and mobile reconnect tests. See patches/codex/README.md
for protocol compatibility details.
