# Android QA Matrix

## Scope

This matrix covers transport reliability and startup-path parity for Android websocket + bridge flows.

## Automated Regression Scaffolding

Run the unit-test suite for the single hybrid runtime:

```bash
./gradlew :app:testDebugUnitTest
```

Current automated checks:

- `SavedServerTransportTest`
  - validate persisted transport selection
- `AppComposerPayloadTest`, `SnapshotExtensionsTest`, and conversation UI tests
  - validate typed composer inputs, snapshot projection, markdown sizing, slash commands, math, and response errors
- `RealtimeWebRtcTransportTest` and `RealtimeWebRtcSessionTest`
  - validate the typed SDP transport and native WebRTC session lifecycle
- OAuth, server pairing, appearance, and home-dashboard tests
  - validate their platform adapters without duplicating the Rust reducer

## Manual Matrix

| Area | Hybrid runtime |
|---|---|
| App launch | App launches and can start a local bridge-backed session |
| Connect local/on-device | Success (`ServerConfig.local`) |
| Connect remote server | Success |
| Manual or saved SSH server | Prompts for SSH credentials, connects through SSH port forwarding, and never attempts `ws://host:22` directly |
| SSH changed identity (saved server and terminal) | Both platforms decode the shared Rust challenge. Verify hostname and IPv6 targets: Cancel preserves the old pin; Replace stores only SHA256 fingerprint and retries successfully. Device acceptance pending. |
| SSH changed identity (guided connect) | Both platforms route a changed host key from guided connect (probe, bootstrap, and SSH bridge paths) to the confirm modal instead of raw marker text; Replace syncs saved servers first so re-pinning works for servers saved this session. A changed key on a never-saved discovery target shows the confirm modal but Replace cannot reconnect it until the server is saved (tracked follow-up). Device acceptance pending. |
| Local transport drop | Reconnect and one-time reinitialize before the next non-initialize RPC |
| Remote transport drop | Reconnect behavior via Rust `AppStore` updates and resumed RPC notifications |
| Slow remote RPC | Shared Rust regression verifies that a 300 ms catalog response does not block unrelated requests or notifications; concurrent transport failures reconnect once, and shutdown drops pending replies. Applies to both platforms. Physical-device tap-to-render validation remains separate. |
| Saved Alleycat relay identity | Shared Rust canonicalizes DNS relay URLs, including older saved pairings. Android 17 ARM64 16K emulator: relay-only cold restarts produced duplicate-endpoint eviction on 2/2 baseline runs; candidate Rust produced 0/3 evictions and connected in 5.5s, 2.4s, and 2.2s. Controlled native-library replacement retained the signed baseline shell and pairing; final signed-artifact and physical-device acceptance remain separate gates. |
| Thread start/resume fallback sandbox | `workspace-write` with `danger-full-access` fallback when Linux sandboxing is unavailable |
| Thread turn pagination (v0.125+ remote) | Conversation opens with the last 5 turns; "Load earlier messages" fetches older 5-turn pages via `thread/turns/list` |
| Thread turn pagination fallback (v0.124 remote) | Capability flips off via response inspection; embedded turns load fully; "Load earlier" is hidden |
| Multi-turn history and rendering | Follow-ups retain existing expansion; authoritative turn boundaries and late provenance are covered by unit tests. Expanded message/tool-group entries use individual lazy rows, with a 500-message fixture. Shared Rust covers stale reads, partial hydration and late completion. See [review and validation contract](../../../docs/MULTI_TURN_REVIEW.md); production-device latency remains a separate acceptance check. |

## Terminal UX Matrix

The terminal screen renders through Ghostty on both platforms; this section
tracks parity between iOS (UIKit + Metal) and Android (Compose + SurfaceView).

| Area | iOS | Android |
|---|---|---|
| Full-screen surface | No bottom composer; the Ghostty surface fills the body | Same |
| Accessory bar | Esc/Tab/Ctrl/arrows/Paste/Clear/Send-to-AI dock above the keyboard via `inputAccessoryView` | Compose row anchored above the IME using `imePadding` |
| Tap-to-toggle keyboard | Single tap (no selection, no link) toggles the keyboard | Same |
| Long-press selection | Long-press seeds word selection; drag extends; handles paint via `TerminalSelectionOverlayView` | Long-press seeds word selection; drag extends; handles painted by a sibling Compose Canvas |
| Edit menu (Copy / Paste / Select All) | `UIEditMenuInteraction` anchored at the selection union rect | Compose floating action menu above the selection |
| Pinch-to-zoom font | `UIPinchGestureRecognizer` clamps to 10–24 pt and re-grids on settle | `ScaleGestureDetector` updates `TerminalConfigPrefs.fontSize` live and re-grids on scale-end |
| BEL haptic | `UIImpactFeedbackGenerator(.medium)` + system sound, throttled 250 ms | `performHapticFeedback(LONG_PRESS, IGNORE_VIEW_SETTING)`, throttled 250 ms |
| OSC8 hyperlink tap | Detected through Rust `linkAtPoint`; opens via `UIApplication.shared.open` | Detected through Rust `linkAtPoint`; opens via `Intent.ACTION_VIEW` |
| Cell-grid math | Driven by Ghostty `surfaceMetrics`; falls back to font-size-aware estimate on first frame | Same path via `nativeSurfaceSize` |
| Resize on rotation / keyboard show-hide | `layoutSubviews` plus `UIResponder.keyboardWillChangeFrame` triggers | `onSizeChanged` re-fires through Compose's `imePadding` insets |
| Mouse-tracking apps (vim / htop) | Single-finger drag forwards to Ghostty when `mouseCaptured` | Same |
| Alleycat remote host | Add Server's Kittylitter QR card opens `AlleycatAddServerSheet`; CameraX + ML Kit scan parses the Alleycat payload via `AlleycatBridge.parsePairPayload`; debug builds expose paste-JSON path; after token-authenticated pairing the sheet calls `serverBridge.listAlleycatAgents`, lets the user choose Codex/Pi/OpenCode, connects with `serverBridge.connectRemoteOverAlleycat`, and persists the token through `AlleycatCredentialStore`; `SavedServerStore.rememberAlleycat` writes `{node_id, relay?, agent}` records, reconnect attaches the encrypted-store token directly, and legacy Alleycat records require a new QR scan. | Same |
| Local Studio host (pairing) | Add Server → **Local Studio** opens the pairing sheet in Local Studio mode. Scanning the QR from Local Studio's Profile → Phone connection, or pasting its Copy connection JSON, saves a server with id `alleycat:local-studio:<nodeId>` and forces a new QR scan if the keychain/prefs entry is missing. | **Verified 2026-07-30 on an API 36 ARM64 emulator.** Copy-connection pairing reached the live Local Studio relay; the shared parser now normalizes the trailing dot emitted by the relay hostname, and the freshly signed release APK reconnected the saved pairing after reinstall. |
| Local Studio host (runtime) | Selecting Local Studio launches the shared Pi bridge against Local Studio's own `pi-agent` home, so the standard model selector, chat/tool timeline, and remote filesystem UI are reused. Permission controls are hidden because the runtime is fixed full-access. | **Verified 2026-07-30 on an API 36 ARM64 emulator.** The signed `1.6.0` release APK connected, resumed and hydrated Local Studio threads, and displayed the completed `ANDROID_LOCAL_STUDIO_OK` turn. The transport proof suite also passed all 11 scenarios, including command output, filesystem access, mid-turn status, reconnect, and final hydration. |

## Plugin `@`-mention parity (follow-up)

iOS ships `@plugin` mentions in the composer: typing `@` now lists installed
Codex plugins above the file results, selecting one inserts an `@<name>`
chip and emits an `AppUserInput.Mention { name, path }` item on
`turn/start`. Path encoding lives in shared Rust (`PluginSummary` UniFFI
record + `list_plugins` on `AppClient`).

Android does not have an `@`-trigger composer popup yet (no inline
`@<file>` autocomplete either), so plugin mentions are a follow-up. The
shared Rust client and the `AppUserInput.Mention` variant are already
wired through the Kotlin bindings, so the platform side just needs a
composer popup, chip row, and send-time append. Track this alongside the
broader composer autocomplete work.

## Suggested Smoke Steps

1. `debug`: connect the local default server, start a thread, send a turn, toggle network off/on, then send another turn.
2. `debug`: force-stop the app, relaunch, and confirm initialization and the thread list recover.
3. `debug`: connect a remote server and run `thread/list` plus `turn/start`.
4. Verify account read/login status refresh still updates UI after reconnect.

## Thinking-indicator Minigame — removed (2026-10-04)

Removed at product direction after the remote-only audit. Neither platform
exposes the experimental setting, launcher or overlay. The dedicated Rust
RPC and generated binding types are removed. The Thinking status indicator,
composer, generic HTML widget renderer and Saved Apps remain.

Validation: regenerated Swift/Kotlin bindings and the Xcode project; no
minigame symbols remain in platform sources or generated bindings. Rust tests
passed (838 passed, 7 ignored); Android JVM tests passed (79). The fast iOS
simulator build, 282 XCTest unit tests and two UI regressions (composer typing
and follow-up history) passed. The composer test's stale `Mode: Default`
assertion was corrected to the existing `Default` label and rerun. Android
native/APK build was attempted but blocked by missing NDK during Ghostty
compilation (`AndroidNDKNotFound`); Kotlin compilation passed with the JVM tests.

## Settings — Server Connection Editor (iOS + Android)

Tapping a saved server row in Settings opens the inline editor. Legacy local
records are removed during migration. Alleycat-paired servers are name-only; everything else allows
mode + host/port/wake-MAC + URL editing. Save persists, Save & Reconnect
disconnects and re-establishes the chosen transport.

| Check | iOS | Android |
|---|---|---|
| Tap saved server row opens editor | `SettingsServerSheet.edit` opens `SettingsServerConnectionEditor` form sheet | Tap or row-menu "Edit" opens `ServerEditSheet` ModalBottomSheet |
| Legacy local server | Removed from saved servers; no shipping local editor entry | Same |
| Alleycat-paired server: only name editable | Editor shows paired-pairing-metadata copy; mode picker hidden | Same — mode picker hidden, message visible |
| Switch to Direct Codex + Save & Reconnect | Persists, disconnects, calls `serverBridge.connectRemoteServer` | Same — `reconnectController.reconnectServer` reconnects via persisted record |
| Switch to WebSocket + Save & Reconnect | Persists, disconnects, calls `serverBridge.connectRemoteUrlServer` with `ws://` or `wss://` | Same — record stores `websocketURL`; reconnect dispatches via Rust `ReconnectController` |
| Switch to SSH + Save & Reconnect | Persists, dismisses editor, opens `SSHLoginSheet`; connect uses `serverBridge.startRemoteOverSshConnect` | Persists, dismisses editor, opens shared `SSHLoginDialog`; connect uses `serverBridge.startRemoteOverSshConnect` |
| Validation errors surface inline | Alert "Invalid Server" with localized reason, dismiss returns to editor | Same — `AlertDialog` with reason; dismiss returns to editor |
| Save (no reconnect) | Persists `SavedServerStore` + calls `store.renameServer`, leaves connection intact | Same |
| Remove server | `SavedServerStore.remove` + closes SSH session + disconnects bridge | Same |

## Sidebar + Picker Parity Checklist (iOS + Android)

### Session Sidebar

The legacy Android `Route.Sessions` and `SessionsScreen` are no longer present
in the current navigation tree. PR #330 intentionally exposes the existing
All Sessions browser and its bounded Load more action on iOS only: Android
needs a new reachable browser, not a link to the removed screen. Shared Rust
hydration limits and protection against pruning other runtimes apply to both
platforms.

Android follow-up acceptance: add a home entry point that browses sessions
across connected servers, loads older results, filters them, and resumes and
pins the selected session. Verify more than 200 sessions per runtime, mixed
Codex/Claude servers, empty results, load failures, and continued live updates
when a CLI-created session is opened. Until then, do not report Android
browser parity as complete.

### Thread List Consistency

- Refresh (`thread/list`) prunes non-authoritative placeholder threads unless they are currently active.
- Notification-only placeholder rows disappear on next refresh once inactive.
- No regressions in thread switching, forking, or session search after placeholder pruning.

### Directory Picker

- Primary action: one-tap `Continue in <last folder>` appears when recents exist.
- Top controls remain visible while list scrolls: connected server chip/status + search.
- Breadcrumb + `Up one level` navigation always reflects current path.
- Bottom CTA is sticky and mirrors path state: `Select <path>` (or disabled helper text).
- Error state exposes both `Retry` and `Change server`.
- `Clear recent directories` requires destructive confirmation.
- Back behavior parity:
  - Android: `Back` navigates up before dismissing sheet.
- iOS: dismiss is blocked while not at root; cancel navigates up first.

### Appearance

- Chat wallpaper can be chosen from the photo library, persists across relaunch, and can be removed from Settings.
- Conversation screen and appearance preview both render the selected wallpaper instead of the fallback theme gradient.

### Conversation Selection

- Settled assistant markdown supports long-press selection/copy.
- Reasoning text supports long-press selection/copy.
- Command output supports selection/copy and still scrolls vertically.
- Error text supports selection/copy.
- Code blocks support selection/copy and still scroll horizontally.
- Markdown links remain tappable after selection support changes.

## Home Dashboard Zoom + Swipe Reply Parity (mirrors iOS b96961b3 + 52ff299d)

Ported in parallel with the iOS "new ui" and "new ui stuff" commits. Each
item must render identically to the iOS `HomeDashboardView` on zoom-1/2/3/4
for the matching state.

| Feature | Check |
|---|---|
| Zoom toolbar button | Top-right of header cycles 1→2→3→4→3→2→1 with icon matching level (ViewQuilt→ViewList→ViewAgenda→ViewStream). Persists across app restart via `DashboardZoomPrefs` SharedPreferences. |
| Pinch-to-zoom | Pinch on home LazyColumn crosses level thresholds (`pinchAccumulator ± 0.4 → 1 level`), emits haptic on transition, does not steal single-finger vertical scroll. |
| Zoom 1 (scan) | Title + StatusDot only; no meta/body/chips. |
| Zoom 2 (glance) | + time · server · workspace meta line. Tool-activity label + pulsing dots appear only when `isActive && isToolCallRunning(hydratedItems)` — pure thinking falls through to server metadata. |
| Zoom 3 (read) | + modelBadgeLine (server icon + server + model + fork/subagent) with trailing inline stats, + user-message quote `>` prefix, + compact tool log (1 row), + response preview capped at 25% screen. |
| Zoom 4 (deep) | Tool log expands to 3 rows; response preview cap rises to 50% screen; preview scroll-anchors to bottom when overflowing. |
| Response preview crossfade | New assistant-block id flip triggers Crossfade on the preview; preserved on empty new-turn assistant items via `displayedAssistantMessage` walking back to last non-empty. |
| TurnStopwatchChip | Live 1Hz tick while turn active (end=null) via `produceState` + `delay(1000)`; static elapsed when ended. Format `<60s → "Xs"`, `<3600s → "Xm" or "XmYs"`. |
| Tool log grouping | Consecutive exploration commands (read/search/listFiles `HydratedCommandActionKind`) collapse into `⌕ Explored N files, M searches, K listings` summary row; other tool kinds render as single-line rows. |
| inlineStats chips | Turn count, tool count, diff `+N/-N`, TurnStopwatchChip, token % (warning tint ≥80%). Left text truncates first; chips stay pinned. |
| recentUserMessage | `>` chip prefix + FormattedText at `LitterFont.conversationBodyPointSize × textScale`. Only shown when message exists and differs from title. |
| StatusDot shimmer | Active state gets both the 800ms alpha pulse AND a 2s linear-gradient sweep overlay. |
| Home hydration | Home list calls `appModel.externalResumeThread(session.key)` — not `client.readThread` — so the server attaches a live listener and cards update without opening the thread. |
| Swipe reply | Right-swipe on home row reveals reply affordance (`SessionReplySwipe` via `SwipeableRow.leadingAction`); past commit threshold opens `QuickReplySheet` modal; send path resumes the thread before `startTurn` to avoid "thread cannot be found" on cold launches. Left-swipe reveals hide (trailingAction). |
| SavedProjectStore | Last-selected server + project persist across app restart via Rust `preferencesSetHomeSelection` / `HomeSelection`. Wired through `LitterApp.kt`. |
| StreamingMarkdownView bodySize | Optional `bodySize` parameter thread through to TextView font size; opt-in by response preview and by direct consumers that need parametric sizing. |
| Streaming render cost | Both platforms extend the cached final markdown chunk for a plain-text append (`StreamingAssistantRenderCache.extendEntry` / `StreamingTextCoordinator.extendFrontier`) and re-parse otherwise. A streamed message must render exactly what a cold parse of the same text renders: iOS asserts this in `StreamingAssistantRenderCacheTests`; on Android, compare the frontier blocks against a `MessageParser.extractRenderBlocksTyped(text)` call on the same text after `StreamingTextCoordinator.clear()`. The fast path only applies when the message has no `http://` / `https://` URL, so a URL split across tokens still autolinks. |

## Tool Call Card Parity Matrix (iOS + Android)

Renderer contract for this release:

- default collapsed for tool cards, except `failed` cards (default expanded)
- header order: icon, summary/title, spacer, status chip, optional duration chip, chevron
- section order: metadata KV, payload sections (`Command/Arguments/Result/Output/Action`), auxiliary sections (`Prompt/Targets/Progress`)
- parse miss fallback: legacy markdown rendering unchanged

| Tool kind | Summary rule | Status chip | Expected sections |
|---|---|---|---|
| Command Execution | stripped command + status/duration suffix | `inProgress`/`completed`/`failed`/`unknown` | Metadata, Command, Output (if present), Progress (if present) |
| Command Output | output label fallback (`Command Output`) when no command | usually `unknown` | Output text/code |
| File Change | first basename + `+N files` | normalized from `Status:` | Metadata, repeated `Change N` metadata + diff/text content |
| File Diff | first path basename when available, else `File Diff` | usually `unknown` | Diff panel |
| MCP Tool Call | `Tool:` value + status suffix/check | normalized from `Status:` | Metadata, Arguments/Result, Error/Progress as available |
| MCP Tool Progress | tool/status fallback or title | usually `unknown` unless merged into MCP call | Progress timeline text |
| Web Search | `Query:` value | usually `unknown` | Metadata, Action JSON |
| Collaboration | `Tool:` value fallback | normalized from `Status:` | Metadata, Prompt text, Targets list |
| Image View | basename from `Path:` | usually `unknown` | Metadata (`Path`) |

Status normalization parity:

- `inProgress`, `in progress`, `running`, `pending`, `started` -> in progress (amber)
- `completed`, `complete`, `success`, `ok`, `done` -> completed (green)
- `failed`, `failure`, `error`, `denied`, `cancelled`, `aborted` -> failed (red)
- anything else/missing -> unknown (neutral)

## Saved Apps (Generative UI persistent apps)

Remote-only adaptation (2026-10-04): shared Rust registers `show_widget` and
`visualize_read_me` on new interactive Codex threads, across WebSocket, SSH,
Alleycat and Slingshot transports. Existing threads keep their server-side tool
registration; start a new conversation to obtain the new tools. Ephemeral
utility threads and other agent runtimes do not gain these tools.

Saved app updates send the existing HTML and state-shape summary over RPC to a
short-lived remote Codex thread, request JSON containing replacement HTML, and
save it on the phone only after a successful, valid response. Device filesystem
paths are never sent as remote working directories. The user state, app UUID,
origin, dimensions and schema version survive HTML replacement. The edit thread
uses read-only sandbox permissions and bounded best-effort archive cleanup.

Validation (2026-10-05): `make rust-test` passed (841 passed, 7 ignored),
including six remote saved-app regressions covering client tool registration,
server-scoped identities, legacy decoding, host selection, HTML/state round
trip and invalid/failed response handling. Response isolation is tested with
colliding thread IDs on different servers and earlier turn IDs. Swift/Kotlin
bindings were regenerated; `make ios-sim-fast`, all 282 iOS XCTest unit tests
(including Saved Apps persistence), and 79 Android JVM tests passed. These
RPC tests use a simulated host; live model generation remains device/host QA.
Android native APK validation is still blocked by the missing local NDK noted
in the minigame-removal validation above.

| Area | Check |
|---|---|
| Bootstrap | `AppClient.setSavedAppsDirectory(SavedAppsDirectory.path(context))` is called once in `AppModel.init`, before any thread starts. Without this, the Rust `show_widget` finalize hook is a silent no-op. |
| Auto-upsert | When the model finalizes a `show_widget` with `app_id = "fitness-tracker"` on a remote Codex thread, Rust upserts by `(originServerId, originThreadId, appId)` into the platform-selected `SavedAppsDirectory` index + `html/<uuid>.html`. No Kotlin-initiated promote call is needed. |
| Saved-as chip | Finalized `WidgetRow` whose `HydratedWidgetData.appId` is non-null renders a compact "Saved as `<slug>`" chip below the WebView (11sp mono, accent slug). Tap resolves `SavedAppsStore.appForSlug(slug, threadId, serverId)` to a UUID and pushes `Route.SavedApp`. Chip is absent when `appId == null` or the widget isn't finalized. |
| Home-row takeover | **Removed.** The takeover row and its `savedAppsByThread` / per-session `sessionApps` feeder pipeline were deleted; no `Takeover` symbol remains under `apps/android`. Saved apps are reachable only via the Saved-as chip and the Apps list. |
| Apps list entry | Settings sheet "Apps → Saved Apps" row is always visible (no flag gate). Pushes `Route.Apps`. |
| Apps list | `AppsListScreen` renders apps newest-updated-first: monogram tile + title + relative timestamp. Swipe-to-dismiss cascades `savedAppDelete`. Empty state explains that saved apps are created automatically. |
| Detail relaunch | Tapping a row (or a Saved-as chip, or a home-row takeover) pushes `Route.SavedApp(uuid)`. `SavedAppScreen` calls `savedAppGet(dir, uuid)` on enter, hydrates the WebView with `wrapWidgetHtml(html, AppStateInjection(stateJson, schemaVersion))`, and registers `__LitterAppBridge` via `addJavascriptInterface`. |
| State persistence | `window.saveAppState(obj)` flows through the bridge into `SavedAppsStore.saveState`, 250 ms trailing-edge debounced per `appId`, landing in `savedAppSaveState`. `SavedAppException.StateTooLarge` logs + swallows. |
| Structured response | `window.structuredResponse({prompt, responseFormat})` returns a Promise that resolves with parsed JSON. Routes through `__LitterAppBridge.structuredResponse(requestId, prompt, schemaJson)` → `AppClient.structuredResponse` on an ephemeral hidden thread (`ThreadStartParams.ephemeral = true`). First call per view session creates the hidden thread; subsequent calls reuse it via `remember(appId) { mutableStateOf<String?>(null) }`. Reply flows back via `webView.evaluateJavascript("window.__resolveStructuredResponse(...)")`. Navigating away + returning resets the cache so a fresh ephemeral thread is created. The hidden thread never appears in the thread list (ephemeral threads are absent from `thread/list`). |
| Cold-launch persistence | Increment a counter in a saved app, `am force-stop`, relaunch, reopen the app. `loadAppState()` pulls `window._initialAppState` from the seeded state JSON — count survives. |
| Update flow | "Update" in the top bar opens `SavedAppUpdateOverlay`. Submit calls `SavedAppsStore.requestUpdate(...)` → `AppClient.updateSavedApp(...)`. WebView dims + shimmer overlay. On success: dismiss + detail re-fetch (state preserved because `replace_html` doesn't touch `state/<id>.json`). Failure: retry inline + toast. |
| Origin server routing | Both platforms use Rust selection: connected origin Codex host → connected active-thread Codex host → deterministic connected Codex fallback. Offline/local/bridge-only hosts are excluded. Ephemeral structured-response caches are scoped to the selected server. |
| View Conversation | Top bar has a chat-bubble icon (`Icons.AutoMirrored.Filled.Chat`) that pushes `Route.Conversation(originThreadKey)`. Origin lookup includes `originServerId`; ambiguous legacy thread IDs do not navigate to an arbitrary host. |
| Rename / delete | Top bar title tap → rename dialog → `savedAppRename`. Overflow "Delete" → destructive confirmation → `savedAppDelete` → pop back to list. |
| Same slug across servers/threads | Model emitting `app_id = "fitness-tracker"` in two different origin threads creates two independent saved apps (distinct UUIDs, separate state files). The Apps list shows both. |
| Regression: timeline widgets with no slug | A `show_widget` call that omits `app_id` (or is pre-R2) renders with the baseline `wrapWidgetHtml(html)` shell, does not trigger auto-save, and shows no Saved-as chip. |
| Regression: thread delete | Deleting an `originThreadId` thread does not affect saved apps; the `View Conversation` button becomes hidden for those apps but update/state flows still work. |

## Timeline WebView shell (SW-A0)

The timeline widget WebView shell (`wrapWidgetHtml` in `ConversationTimeline.kt`) is kept at parity with iOS's `WidgetWebView.buildShellHTML`. The shell is loaded **once** per WebView; subsequent content changes push through `window._setContent(html)` / `window._runScripts()` via `evaluateJavascript`, never a full page reload. This is the foundation for SW-A1 (partial widget-HTML streaming) and the fix for the prior "widgets pop in" behavior.

| Area | Check |
|---|---|
| Shell parity | `wrapWidgetHtml` emits the full iOS-parity document: theme vars in `:root`, `@keyframes _fadeIn` + `onNodeAdded` fade-in hook, morphdom CDN (`morphdom@2.7.4/dist/morphdom-umd.min.js`), and the `_morphReady` / `_pending` / `_setContent` / `_runScripts` / `_reportHeight` / `_attachHeightObserver` / `sendPrompt` / `openLink` JS. App-mode injection splices before `window._morphReady = false;`. |
| Bridge | JS posts `{_type, ...}` messages through `__postWidgetMessage`, which routes: `saveAppState` → `__LitterAppBridge`, `height`/`sendPrompt`/`openLink`/`ready` → `__LitterWidgetBridge`, with the iOS `webkit.messageHandlers.widget` fallback (no-op on Android). Both bridges coexist on the saved-app WebView. |
| Shell loads once | `AndroidView.factory` calls `loadDataWithBaseURL(..., wrapWidgetHtml(""), ...)` exactly once. `WebViewClient.onPageFinished` flips a per-WebView `widget_webview_shell_ready` tag to `true` and flushes any buffered HTML through `pushWidgetContent`. |
| Content push | Subsequent HTML changes in `AndroidView.update` call `pushWidgetContent(webView, html, runScripts = isFinalized)`, which runs `webView.evaluateJavascript("window._setContent('${escaped}'); window._runScripts();", null)`. `escapeJsString` matches iOS's `escapeJS` (backslash, single-quote, newline, CR, `</script>` → `<\/script>`). |
| Pre-ready queue | HTML changes that arrive before `onPageFinished` are stored on the WebView via the `widget_webview_pending_html` tag; `onPageFinished` flushes them. No content is dropped. |
| Dynamic height | `__LitterWidgetBridge.height(px)` posts the reported height through a main-thread `Handler`, clamped to `[200dp, 720dp]`. Compose's `mutableStateOf<Dp>(initial)` drives the WebView's `.height(widgetHeight)` modifier and animates smoothly as the widget grows/shrinks. Initial seed is the declared `data.height`. |
| sendPrompt | A widget button that calls `window.sendPrompt(text)` routes through `__LitterWidgetBridge.sendPrompt` → `onWidgetPrompt` callback in `WidgetRow` → `ConversationScreen` builds an `AppComposerPayload` and calls `appModel.startTurn(threadKey, payload)` — parity with iOS's `sendWidgetPrompt` which also submits a turn immediately. |
| openLink | Widget call to `window.openLink(url)` routes through the bridge to an `Intent.ACTION_VIEW` in the host Activity, opening the URL in the default browser. |
| Save-as-App bubble | `show_widget` finalize → auto-save (Rust-side) → Saved-as chip appears under the WebView. Shell lifecycle unchanged. |
| Saved app detail | `SavedAppScreen` uses the same shell through `wrapWidgetHtml("", AppStateInjection(...))` loaded once, then pushes `payload.widgetHtml` via `pushWidgetContent`. `loadAppState`/`saveAppState` still work. State persists across cold relaunch. |
| Regression: finalized timeline widget | An existing finalized `show_widget` renders identically to pre-refactor — fade-in animation, tap routing, state persistence of saved-app mode all preserved. |

## Realtime Voice (WebRTC transport)

Replaces the prior WebSocket + base64-PCM audio pump with a platform-native WebRTC peer connection on both iOS and Android. Upstream `thread/realtime/start` receives a client offer SDP via `AppRealtimeStartTransport.Webrtc`; the app-server responds with an answer SDP via `ThreadRealtimeSdpNotification`. All other realtime notifications (transcripts, item-added, handoff, closed, error) continue over the existing RPC WebSocket — only the audio byte path moved to the peer connection.

| Area | iOS | Android |
|---|---|---|
| Start request carries Webrtc transport | `AppStartRealtimeSessionRequest.transport == .webrtc(sdp:)` with a non-empty offer SDP (log at session start) | Same — `AppRealtimeStartTransport.Webrtc(sdp)` |
| Answer SDP applied | `AppStoreUpdateRecord.realtimeSdp` → `RealtimeWebRtcSession.applyAnswer(_:)` → `setRemoteDescription` succeeds | `AppStoreUpdateRecord.RealtimeSdp` → `RealtimeWebRtcSession.applyAnswer` → `setRemoteDescription` succeeds |
| Peer connection reaches connected state | `RTCPeerConnectionState.connected` observed via delegate | `PeerConnection.IceConnectionState.CONNECTED` observed |
| Bidirectional audio | Assistant voice plays back; mic input produces responses | Same |
| Transcript deltas (RPC path) | `ThreadRealtimeTranscriptDelta`/`Done` notifications still render | Same |
| Client-controlled handoff during voice | `HandoffManager` receives `HandoffRequested`, `resolveHandoff` / `finalizeHandoff` round-trip completes | Same |
| Dynamic tool call during voice | Argument deltas stream via RPC `ConversationItemAdded`; tool output returns via `resolveHandoff` | Same |
| Session stop | `RealtimeWebRtcSession.stop()` closes peer + data channel, deactivates `RTCAudioSession` | `stop()` disposes peer, restores audio mode, abandons audio focus |
| Session cycle (start/stop x5) | No leaked peer connections, microphone releases between sessions | No leaked peer, mic indicator clears between sessions |
| Known non-blockers | Per-frame input/output meter animation no longer drives — requires `RTCRtpReceiver.stats` polling to restore (follow-up) | Same flat meter behavior; speaker toggle currently stubbed to a boolean — follow-up to honor runtime routing |
| Regression: custom AEC path | Retired — `codex-ios-audio` crate + `AecBridge.swift` / `VoiceSessionAudioCodec.swift` were deleted; libwebrtc AEC3 handles echo cancellation natively | Retired — `AecBridge.kt` deleted; `JavaAudioDeviceModule` enables the hardware AEC + NS |
| Regression: SSH-tunneled codex server | RPC still flows through SSH; WebRTC peer goes direct to OpenAI edge from device. If client runs in fully air-gapped network, realtime voice will not establish | Same |
| Theme hex colors preserve CSS alpha | `litterHexRGBA` parses #RGB/#RGBA/#RRGGBB/#RRGGBBAA with alpha last; swatch/index colors keep the alpha byte; theme tokens strip alpha at decode (`ThemeDefinition.sanitizeHex`) so app colors stay opaque (`HexColorTests`) | `colorFromHex` moves the alpha byte to the ARGB front instead of dropping it; theme tokens strip alpha via `tokenColorFromHex` to match iOS and the generated Material schemes (`ThemeColorTest`) |

## English / Simplified Chinese localization

| Area | Check |
|---|---|
| Initial language | A fresh install follows the device language for English and Simplified Chinese; other locales fall back to English. |
| In-app switch | Settings > Language switches between English and Simplified Chinese, recreates the activity, and persists across a cold launch. |
| Main workflows | Home, projects, sessions, directory picker, conversation composer/timeline/info, approvals, model and permission panels, discovery, account, saved apps, wallpaper, tip jar, voice, terminal, pet, experimental, and debug screens display the selected language. |
| Accessibility | Icon content descriptions for navigation, composer actions, images, voice, models, wallpaper, and server actions use the selected language. |
| Background surfaces | Firebase turn notifications, the active-turn widget, and the pet overlay menu use the persisted app language even when launched outside the main activity. |
| Format strings | English and `values-zh-rCN` contain identical keys and positional format placeholders. Dynamic server/model/path/error values remain unchanged. |
| Language change during work | Start a turn, switch language, and verify the active session remains intact after Activity recreation; newly posted notifications and refreshed widget content use the new language. |

## Message link menus (iOS PR #322)

This PR updates iOS's Hairball renderer and native context menu. Android's
Markwon renderer needs a separate parity verification: bare HTTP/HTTPS URLs
must be tappable, existing Markdown links must remain valid, and code must
remain literal. The per-link copy action is an Android follow-up; verify URL
deduplication, the five-link cap, and distinct labels for ports, queries, and
fragments when adding it. No Android execution was covered by this iOS change.

## 2.1.1 stability candidate

- Shared SSH setup now discovers tool directories without executing login/rc
  scripts. Regression fixture verifies no shell startup side effects; Nix
  path discovery remains covered. Real SSH and Kittylitter device acceptance
  is pending.
- Android Ghostty selects epoll directly to avoid seccomp's fatal io_uring
  probe (#354). Terminal open/input/close/reopen acceptance on Android hardware
  is pending; compilation alone does not close this gate.
- Android Ghostty now bundles GLAD and explicitly links EGL/GLESv3. Verify the
  native library actually loads: the older 2.1.0 APK fell back to text output
  after an unresolved `imgl3wProcs` symbol, masking native renderer failures.
- Android 17 / ARM64 / 16 KB emulator: libraries load, opening and reopening
  Terminal does not trigger SIGSYS, and basic command input/output passes via
  both Run and the keyboard action. Native surface creation fails on OpenGL ES
  3.1. The fallback is line-oriented and does not provide full ANSI screen
  semantics. Physical-device and native-renderer acceptance remain pending.
- Home server/session projections are memoized by their actual inputs rather
  than the entire snapshot, avoiding repeated sorting on unrelated deltas.
- Conversation rows use stable turn identities when earlier pages are inserted,
  matching iOS. Verify the visible row and expansion state survive pagination.
- Both platforms set Codex thread start/resume/fork configuration to disable
  login-shell tools and shell snapshots; shared Rust regression tests cover the
  three operations and preservation of unrelated settings and other runtimes.
- App Store/Play release acceptance is tracked separately from build success.

## Harness catalog and settings refresh (2.1.2)

Implemented on both iOS and Android through shared Rust. Device acceptance is pending for this revision.

| Check | Required result on both platforms |
|---|---|
| Cold model picker | Native catalog available before creating a thread; custom and plugin IDs preserved |
| Refresh/reconnect | Catalog expires after 60 seconds, incomplete refresh retries after 5 seconds; failed runtime retains prior choices; old connection cannot publish into replacement |
| Reasoning selection | Native supported values only; persistent and custom values round-trip |
| OMP isolation | Separate runtime, model catalog, and config.yml; Pi settings are unchanged |
| Harness settings | Searchable lazy list, typed edits, native source/scope, enforced read-only constraints, authoritative save read-back |
| SSH settings | Claude/Pi/OMP values read and written on the remote host with credential fields preserved and redacted |
| Streaming | Unchanged server/runtime projections do not invalidate the settings list |

## Model picker layout

Both platforms render the composer model picker from the same Rust fields (`ModelInfo.entryKind`, `pickerName`, `providerLabel`). iOS verified on simulator against a paired kittylitter host; Android is compile-checked in CI only.

| Check | Required result on both platforms |
|---|---|
| Root | Current selection, up to 4 recents, one row per harness with counts, then Options (reasoning, fast). Plan mode is on this page only before a thread exists; an existing thread uses the same page without it |
| Harness page | Modes and plugin modes listed separately from models; multi-provider catalogs over 30 models start folded with the selected provider open |
| Search | Matches every whitespace token across harness, provider, and model names; results capped at 150 with a total count |
| Modes | Amp `low`/`medium`/`high`/`ultra` are `Mode`, other Amp entries `PluginMode`; never shown as models |
| Host execution | Discovery and settings launch headless processes without terminal windows or Dock helpers |

Validation for the 2.1.2 candidate: shared Rust library tests passed (823 passed, 3 existing manual/live tests ignored). The final `a5aaa9f6` Android library built successfully; all 58 unit tests and six settings/navigation instrumentation tests passed on Android 17. These cover unset strings/booleans/enums, managed policy, and MainActivity Settings → Harnesses navigation. Logo/splash animation state stays in draw/layer scopes using the standard Compose infinite-animation clock. The focused iOS harness settings UI test also passed against the ABI-compatible simulator library, including authoritative fixture readback and read-only controls. Final iOS artifact/input validation and store release remain pending.

## Remote-only Codex branch

Both mobile platforms disable embedded Codex startup, automatic local reconnect,
local terminal options and rootfs bootstrap. Remote WebSocket, SSH, Alleycat,
Slingshot, approvals and canonical store updates stay in shared Rust. Voice
selects a connected remote host; optional handoff extensions require host support.
Validate remote attach, turn streaming, approvals and reconnect on a device.

Validation on 2026-10-02: shared Rust tests passed (857 passed, 7 existing
manual/live tests ignored), iOS XCTest passed (281 unit tests plus the harness
settings navigation/editing/read-only UI test), and Android JVM
unit tests passed (82 tests). A loopback WebSocket integration test verifies
initialize, typed RPC, notifications and server-request replies. iOS device
and simulator static libraries compiled; the Release archive includes Watch
and Live Activity and launches on the simulator. Android JNI/device and real
remote-host end-to-end checks remain for device QA.

Remote voice pins are scoped by server ID on both platforms. iOS handoffs use
the voice session's host; CarPlay resumes connected remote threads. Account
screens show the selected remote host, and empty terminal screens prompt for
a computer connection rather than an embedded runtime bootstrap.

## Original interface restored (2026-10-03)

Both platforms use the pre-upgrade themes, typography, glass controls, home
illustrations, session cards, chat bubbles, composer and settings structure.
The upstream Quiet/ChatGPT visual redesign and its unused components are
removed. Model harness/provider/mode classification and whole-catalog search
remain available through the original picker entry points and theme.

Shared Rust history fixes, title cleanup, saved-computer sync, device-only
tokens and reconnect improvements are retained. iOS keeps isolated composer
drafts, off-main launch/attachment work, no hidden keyboard warmup, stable
message scroll identities and expansion across turn acknowledgements. Android
keeps system font scaling and Android 15 system-bar compatibility, viewport
prefetch, paged rendering and bounded hydration while retaining the original
cards. Both platforms retain individual reasoning/tool controls; finishing a
turn does not replace its tool cards with a collapsed work-chain summary.

Validation: 281 iOS unit tests and three UI regressions passed (follow-up
history, 500-message scrolling and harness settings edits/read-only policy);
79 Android JVM tests passed. Design-specific section grouping assertions were
removed along with the discarded section UI. Rust runtime sources and mobile
FFI artifacts are unchanged from the previously validated remote-only build.

## Remote-only and conversation review (2026-10-04)

- Saved-server migration on both platforms retires legacy embedded-runtime
  records by ID/source, retaining explicit loopback tunnels and paired hosts.
  The shared Rust home cache also drops unreachable embedded-session previews.
- Android excludes cached Alpine/proot assets and native executables from
  packaging, including incremental builds from pre-migration checkouts.
- Android transcript regression coverage verifies that tool rows keep their
  identities and remain visible when a streaming turn finishes.
- Android message actions report edit/fork RPC failures instead of allowing
  an uncaught edit exception or silently hiding fork errors. A successful fork
  opens its conversation, matching iOS. Exercise against a connected host for QA.
- iOS restores the runtime-disable guard for unit tests and conversation UI
  fixtures, preventing saved-host reconnects during isolated UI tests.

Validation: `make rust-test` (842 passed, 7 ignored), remote-client tests
(2 passed), `make test-android` (79 passed), iOS simulator unit tests
(282 passed), and three iOS UI regressions (follow-up history, 500-message
scrolling, rapid composer input) passed. A Gradle merge check with temporary
legacy Alpine/proot fixtures confirmed exclusion from assets and JNI outputs.
Android device UI and real-host edit/fork flows still need end-to-end QA.

## SSH chat recovery (2026-10-04)

The shared Rust SSH client re-authenticates a closed transport before rebuilding
its app-server proxy/tunnel, serializing concurrent reconnects and keeping the
original verified host key pinned. Exhausted short retries now enter a bounded
retry interval instead of terminating the session worker. Offline actions fail
immediately during that interval, and explicit disconnect cancels recovery.
Whole-session replacement also restores active/tracked thread subscriptions.

Validation: Rust tests cover a real loopback SSH server dropping its connection,
concurrent recovery, forwarding through the original local port after recovery,
changed host-key rejection, explicit disconnect, and RPC recovery after five
failures without navigation or a reachability callback. `make rust-test`
passed 846 tests (7 ignored), the final `make ios-sim-fast` build passed, and
the iOS simulator unit suite passed 282 tests during integration validation.
iOS is the primary integration-validation target for this fix; Android uses
the same Rust implementation and needs a later device recovery pass.

## Session re-entry, permissions and weak-network messages (2026-10-08)

| Regression | iOS | Android |
|---|---|---|
| Thinking label moves during session re-entry | Gradient uses a timeline without a repeating layout animation; footer disables inherited animations | Compose shimmer changes brush coordinates only; existing tail-follow policy avoids repeated scrolling on thinking ticks |
| Full access disappears after metadata refresh/reconnect | Shared reducer preserves omitted permission fields; native hydration preserves the last known selection and explicit user overrides | Same shared reducer and matching native hydration behavior |
| Permissions cannot be changed in historical sessions | Composer exposes read only, auto and full access for supported runtimes | Composer exposes the same presets for supported runtimes |
| Latest user message disappears while AI streams | Shared reconciliation requires matching turn identity before replacing a local message with an identical historical message | Same shared Rust implementation |
| Lost turn/start reply after execution started | Server turn/item events confirm the send; retain its message and avoid restoring a duplicate draft. Unconfirmed failed sends still roll back | Same shared Rust implementation |
| Screenshot thumbnail dragged into composer | UIImage and PNG/JPEG/HEIC/GIF image data go through the attachment path on the text field, home composer and conversation composer | iOS-specific screenshot drag workflow; Android attachment paths remain unchanged |

Validation: `make rust-test` passed 849 tests (7 ignored), `make test-android`
passed 80 tests, and the iOS simulator unit suite passed 286 tests. Added shared
regressions cover a lost turn/start reply before confirmation, after TurnStarted,
and after a completed send with its user-item echo. An iOS test loads an actual
PNG item provider and verifies attachment delivery on the main thread without
changing composer text. Real-device screenshot-thumbnail dragging, repeated
navigation during thinking, and real-host airplane-mode recovery remain manual QA.
The latest iOS simulator Rust staticlib build and two UI regressions (rapid
composer input and follow-up history preservation) also passed.

## Remote-only dependency and legacy-flow audit (2026-10-04)

- The resolved mobile Cargo graph excludes `codex-core`, `codex-app-server`,
  iSH and proot. Protocol and remote transport crates remain required.
- Removed Alpine/proot build targets, download/build scripts, unused Android
  bootstrap, iSH vdso validation and CI rootfs archive inputs. Keep Android's
  asset/JNI exclusions and Make cleanup for artifacts from older checkouts.
- The iOS Rust build now leaves patch ownership to `sync-codex.sh` instead of
  maintaining cleanup logic for historical local-execution patches.
- Legacy local connect/reconnect rejects before reusing, replacing or
  disconnecting sessions. The public binding signatures remain unchanged.
- Shared tests: 845 passed, 7 ignored. Remote transport tests: 2 passed.
  Android JVM tests: 79 passed; Kotlin binding generation completed. Workflow
  YAML parsing, edited shell syntax and `git diff --check` passed.
- No new iOS/Android native build or real-host/device end-to-end run was made
  for this audit. Voice, remote filesystem, pairing and release packaging
  still require that acceptance. Saved Apps were subsequently adapted as
  described above; live-host/device acceptance remains separate.

## Asynchronous questions (2026-11-12)

Both platforms reuse the existing themed question panel for `request_user_input_async`.
Rust preserves `AgentMessage.questions`, uses desktop-compatible question IDs and
reply envelopes, and projects pending questions through the existing input-request
snapshot. Legacy blocking input requests keep their RPC response path.

| Behavior | iOS | Android | Shared regression coverage |
| --- | --- | --- | --- |
| Choices plus a custom answer; first async choice selected by default | Existing capsule controls and text field | Existing choice controls plus text field | Typed question hydration and literal freeform reply |
| Skip without a model message | Existing panel button | Existing panel button | Local resolution; no server-request response |
| Failed answer delivery | Retain draft; show error | Retain draft; show error | Failed steer leaves request pending |
| Navigate away or collapse | UI drafts keyed by request | UI drafts keyed by request | Runtime questions remain in Rust |
| Another client answers; history is replayed | Shared state removes question | Shared state removes question | Reply-before-question, skip/replay, server isolation |
| Turn finishes while an answer is pending | Keep question and draft available | Keep question and draft available | Existing pending answers survive; closed historical questions do not reopen |

Mobile retains an already-visible unanswered question after turn completion, so a
user returning from the background can send the answer in a new turn. This adapts
the upstream terminal's draft-recovery behavior to the existing mobile composer.
Live-host/device acceptance remains manual: ask several questions, edit a custom
answer, background/reopen, answer from a second client, and retry after a network
failure. The upstream TUI/desktop visual layout is not imported.

Validation: shared Rust tests passed (856 passed, 7 ignored), Swift/Kotlin bindings
regenerated, and generated Swift bindings type-checked. The iOS fast simulator
build passed on iPhone 18 Pro (iOS 27.0); the freshly built app was installed and
launched successfully. Android native validation was stopped during dependency
download at the user's request; this change's completed native validation is iOS-only.
