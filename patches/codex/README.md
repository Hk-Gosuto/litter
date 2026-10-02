# Remote Codex compatibility patches

The mobile apps connect to a Codex app-server running on another computer.
They no longer link `codex-core`, `codex-app-server`, their embedded execution
hooks, iSH, or Android proot. The shared store and platform UniFFI surface stay
in `codex-mobile-client`; the remote transport is `litter-remote-client`.

Validated protocol base: `rust-v0.162.0-alpha.4` (`fe3894843141854f0d8e4298ece89fdeedd089c4`).

`apps/ios/scripts/sync-codex.sh` applies only:

- `remote-protocol-compat.patch`: keeps the existing mobile wire extensions
  (dynamic-tool argument deltas, realtime handoff/dynamic-tool request types,
  optional permission fields) available to the shared reducer and typed RPC
  generator. This patches protocol dependencies only; it does not install
  handlers in a remote server. Extension RPCs require a server that supports
  them, as before.
- `absolute-path-cross-platform.patch`: accepts Windows absolute paths on
  mobile when reading responses from a Windows host.
- `remote-crypto-compat.patch`: retains age 0.11.x in the protocol dependency
  graph. age 0.12.1 introduces ml-kem 0.2.1 whose prerelease kem requirement
  resolves to an incompatible stable kem 0.3 used by mobile SSH. Cargo.lock
  records the working version; no registry checkout is modified.

The other patch files are historical and are not applied in this branch.
Removing the local runtime does not establish that upstream fixed their
original bugs (including cold fork lineage); those server-side fixes must be
assessed independently on the host.

The Apache-2.0 remote transport snapshot in `shared/rust-bridge/litter-remote-client`
is adapted from this same upstream tag, including the previous Litter generic
`JsonRpcWire`/SSH stream changes. Keep its source attribution and LICENSE when
updating it. Protocol-only upgrades do not require rebuilding an embedded
Codex server or migrating local execution hooks.

After updating the gitlink, refresh active patches against pristine upstream,
run the sync script twice, regenerate both bindings, and run remote transport,
Slingshot and mobile-store tests. Generated Rust stays local-only.
