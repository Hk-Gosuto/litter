# Codex submodule patches

`apps/ios/scripts/sync-codex.sh` applies these patches to
`shared/third_party/codex` during a build.

## Codex 0.146.0

`litter-mobile-compat-0.146.0.patch` is generated against the upstream
`rust-v0.146.0` tag (`e363b08c9175ac1cbe5893615dd2cb9ddf95043b`).

The previously separate patches are consolidated for this tag because realtime
voice, dynamic tools, and app-server event forwarding now modify overlapping
protocol and event-dispatch matches. A single patch gives `sync-codex.sh` a
reliable apply/reverse-apply check.

The patch contains these Litter-specific behaviors:

- Mobile exec hooks for iOS plus Android tool resolution and argv preflight.
- A V8-free `code-mode` stub for iOS, Android, and Linux mobile builds.
- A two-second shell snapshot timeout on mobile.
- Android installation-id persistence without an unsupported file lock.
- Cross-platform absolute-path deserialization.
- Generic app-server JSON-RPC wires and websocket-over-stream support for
  SSH/Alleycat transports.
- Authoritative approval and sandbox fields on `thread/read`.
- Serde defaults for approval timestamps.
- Streaming dynamic-tool argument deltas through protocol, app-server, core,
  rollout, MCP, and TUI event matches.
- Realtime server hints, client-provided dynamic tools, client-managed handoff
  resolve/finalize RPCs, and function-call result forwarding.
- The `pqclean` ML-KEM-768 backend for Clatter. Codex 0.145's default
  pre-release RustCrypto KEM dependency cannot coexist with the stable KEM
  dependency selected by the mobile SSH stack.

Codex 0.146 already includes forward-compatible reasoning effort variants,
client-managed realtime handoff plumbing, and WebRTC auth handling. Their old
standalone patches are therefore not carried forward.

## Refreshing

After checking out a new upstream Codex tag:

1. Apply or reimplement the behaviors above in the submodule.
2. Include new files with `git -C shared/third_party/codex add -N <path>`.
3. Regenerate the patch:

   ```sh
   git -C shared/third_party/codex diff --binary \
     --output=../../../patches/codex/litter-mobile-compat-<version>.patch HEAD
   ```

4. Verify both directions:

   ```sh
   git -C shared/third_party/codex apply --reverse --check \
     ../../../patches/codex/litter-mobile-compat-<version>.patch
   git -C shared/third_party/codex diff --check HEAD
   ```

5. Update `PATCH_FILES` in `apps/ios/scripts/sync-codex.sh`.
