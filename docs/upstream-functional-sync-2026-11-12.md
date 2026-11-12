# Functional upstream sync — 2026-11-12

This pass preserves the existing iOS interface. Codex stays pinned at
`fe3894843141854f0d8e4298ece89fdeedd089c4` (`rust-v0.162.0-alpha.4`);
the remote client receives selected compatible changes rather than a wholesale
update to upstream `main` (`6bfdc75f0d9e6fae313234f694839866d7c63c39`).

## Implemented in the mobile client

| Upstream | Adaptation |
| --- | --- |
| Litter `1332a1e6e`, `7c2756ad` | Remove the launch keyboard warmup. Do not create a hidden first responder or change the current navigation/composer appearance. |
| Codex `8c0f701167` / #53049 | Connection-bound, single-use reply handles. The session worker gives incoming server requests unique mobile IDs and translates resolved notifications back to these IDs. Reconnect invalidates old handles; a stale UI action cannot target a new connection reusing a wire request ID. |
| Codex `95db2cd5d5` / #53155 | Retain the event receiver until the remote worker finishes closing its WebSocket. |
| Codex `13b1318f2e` / #57218 | Negotiate `zstd-stream-v1` on direct, tunneled, and Unix WebSockets. Keep per-direction compression history, bound decompressed messages, respect compressed-frame limits, and discard a connection after write failure. No acknowledgment means the existing plain-text protocol. JSON-line transports remain unchanged. |
| Codex `daa69c411c` / #59776 | Expose `AppClient.list_models_for_provider`. Discover providers, then require a selector-specific typed rejection for a unique nonexistent provider before trusting `model/list`. This distinguishes discovery-only versions from selector-capable versions. Paginate the selected provider, retain provider labels, reject repeated cursors/session replacement/in-session reconnect, and leave the default catalog untouched. |

`patches/codex/provider-model-list.patch` adds the narrow protocol shapes needed
by the client and is applied by `apps/ios/scripts/sync-codex.sh`. It does not
implement provider discovery on an older remote server. The provider list is
read only; credentials and provider configuration are not written or logged.
The new operation is available through the generated binding without adding
new model-picker controls.

## Remote Codex updates still required

Mobile does not link `codex-core` or an app server. The following fixes execute
on the connected computer and cannot be delivered by changing an iOS binary.
No connected computer was upgraded during this pass. Upgrade the host Codex
using a release containing these commits, keeping their prerequisite changes:

| Upstream | Host-side behavior |
| --- | --- |
| `bee28e8a06` / #50427 | Cap persisted command output at 64 KiB, retaining UTF-8-safe beginning/end segments. The existing mobile display cap remains separate. |
| `39b42aaacd` / #60519 | Restore unfinished historical tool calls as aborted work, including forks with pending tool calls. |
| `83e3d63ba1` / #60700 | Finalize aborted history turns and ignore unrelated late aborts. |
| `c9750d7f50` / #58221 | Persist completed long realtime V3 transcript chunks before handoffs/end-of-session flushing without duplicating speech. |
| `ef186f812a` / #57817 | Detach realtime sideband observers without terminating the frontend-owned voice session. |

Do not cherry-pick these server commits into the mobile transport crate: they
depend on the execution engine, history store, or realtime backend. Updating
only the protocol submodule would not change an already-installed host binary.

## Validation

- Shared mobile Rust after review fixes: 866 passed, 7 ignored.
- Remote transport: 6 passed, including negotiated compression over a real
  loopback WebSocket, 32-byte frame fragmentation, codec history, old-server
  plain-text fallback, stale replies, duplicate replies, and shutdown delivery.
- Provider tests cover discovery, cursor paging, old servers, missing providers,
  repeated cursors, and preservation of the default cache.
- Normal mobile dependency graph excludes `codex-core`, `codex-app-server`, and
  `codex-app-server-transport`.
- Swift/Kotlin bindings regenerated; fast iOS simulator build passed on iPhone
  18 Pro (iOS 27.0) after the final protocol adaptation. Android native builds
  were not run, following the user's iOS-only validation scope.
- Live-host acceptance still requires both an older plain-text server and a
  newer compression/provider-capable server. The transport compatibility tests
  use controlled loopback servers rather than modifying a connected computer.

## Review follow-ups

| Reviewed requirement | Fix and evidence |
| --- | --- |
| Old blocking approvals/questions must not block new requests after reconnect | Expire retained identities into `serverRequest/resolved` notifications on every worker reconnect/exit. Invalidate connection-owned requests directly on whole-session replacement, where the old event reader has already been detached. Tests preserve async questions and other servers, verify duplicate completion and wire-ID reuse, and retain notification identity after a successful write. |
| Closing one iOS question must not hide all other questions or lose drafts | Project the complete relevant request list; select the first expanded question. If all are collapsed, retain a reopenable entry and cycle through collapsed requests. Drafts remain keyed by request. Submission and close actions capture the displayed request rather than re-reading mutable selection. Native tests cover multiple closes/reopens and resolved entries. |
| Authoritative history must apply normal-prompt supersession | Resolve replies first, then supersede only source questions known to precede a newer normal prompt, and register only later actionable questions. Duplicate user-item completion cannot clear a newer question. A standalone public `thread/read` probe changed from one stale question to zero after the fix. Older pages retain questions outside their page. |
| Lost mutation replies must not automatically repeat model input | Retry only an explicit allowlist of read-only RPCs. Actual worker tests lose `turn/start` and `turn/steer` replies, require errors rather than successful resubmission, and verify later read requests work after reconnect. Authoritative answer envelopes still resolve the pending question. |
| Provider discovery alone must not imply selector support | Tests include discovery-only servers that ignore the selector, unrelated/authentication failures, missing providers, managed provider policy, real populated paginated catalogs, and default-cache isolation. A loopback test reconnects after capability probing to an older peer returning default models and verifies the result is rejected. |
| Preserve current rendering and model localization | Home footer first-frame/zoom regression tests and existing model-picker projection tests remain in the iOS acceptance set. English/Chinese Terra descriptions continue to resolve through the existing localization maps. |

Final iOS acceptance after the review fixes: 12 selected native tests passed
(question presentation/drafts, home first-frame layout, conversation projection,
model-picker catalog), with no failures. The final public history-read probe
retains one question before the newer user prompt and zero afterward. No
additional definite defect was found in the final follow-up source review.

Release acceptance: an unsigned iPhoneOS archive was built from the final
reviewed sources with build number `210020003`. The resulting
`artifacts/unsigned-ios/Litter-2.1.9-210020003-review-fixes-unsigned.ipa`
passed ZIP integrity verification and recursive signature removal checks for
all five Mach-O files. The Watch companion is included and has the matching
build number. A SHA-256 sidecar is stored beside the ignored local IPA.
