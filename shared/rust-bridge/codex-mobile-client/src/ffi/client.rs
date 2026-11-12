use crate::MobileClient;
use crate::ffi::ClientError;
use crate::ffi::shared::{blocking_async, shared_mobile_client, shared_runtime};
use crate::local_runtime_instructions::splice_local_runtime_developer_instructions;
use crate::next_request_id;
use crate::types;
use base64::Engine;
use codex_app_server_protocol as upstream;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

#[cfg(test)]
#[path = "provider_model_tests.rs"]
mod provider_model_tests;

async fn rpc<T: serde::de::DeserializeOwned>(
    client: &MobileClient,
    server_id: &str,
    request: upstream::ClientRequest,
) -> Result<T, ClientError> {
    client
        .request_typed_for_server(server_id, request)
        .await
        .map_err(|error| ClientError::Rpc(error.to_string()))
}

async fn rpc_runtime<T: serde::de::DeserializeOwned>(
    client: &MobileClient,
    server_id: &str,
    runtime_kind: types::AgentRuntimeKind,
    request: upstream::ClientRequest,
) -> Result<T, ClientError> {
    client
        .request_typed_for_server_runtime(server_id, runtime_kind, request)
        .await
        .map_err(|error| ClientError::Rpc(error.to_string()))
}

fn ensure_settings_session(
    client: &MobileClient,
    server_id: &str,
    session: &Arc<crate::session::connection::ServerSession>,
) -> Result<(), ClientError> {
    let current = client
        .get_session(server_id)
        .map_err(|e| ClientError::Rpc(e.to_string()))?;
    if !Arc::ptr_eq(&current, session) {
        return Err(ClientError::Rpc(
            "Connection changed while accessing settings; refresh before editing".into(),
        ));
    }
    Ok(())
}

async fn settings_rpc(
    client: &MobileClient,
    server_id: &str,
    session: &Arc<crate::session::connection::ServerSession>,
    runtime_kind: types::AgentRuntimeKind,
    request: upstream::ClientRequest,
) -> Result<serde_json::Value, ClientError> {
    ensure_settings_session(client, server_id, session)?;
    session
        .request_client_for_runtime(runtime_kind, request)
        .await
        .map_err(|e| ClientError::Rpc(e.to_string()))
}

async fn read_runtime_settings(
    client: &MobileClient,
    server_id: &str,
    session: &Arc<crate::session::connection::ServerSession>,
    runtime_kind: types::AgentRuntimeKind,
) -> Result<crate::runtime_settings::RuntimeSettingsSnapshot, ClientError> {
    let params = serde_json::from_value(serde_json::json!({"includeLayers":true}))
        .map_err(|e| ClientError::Serialization(e.to_string()))?;
    let mut response: serde_json::Value = tokio::time::timeout(
        Duration::from_secs(15),
        settings_rpc(
            client,
            server_id,
            session,
            runtime_kind.clone(),
            upstream::ClientRequest::ConfigRead {
                request_id: upstream::RequestId::Integer(next_request_id()),
                params,
            },
        ),
    )
    .await
    .map_err(|_| ClientError::Rpc("Runtime settings read timed out".into()))??;
    if runtime_kind == "codex" {
        let request = upstream::ClientRequest::ConfigRequirementsRead {
            request_id: upstream::RequestId::Integer(next_request_id()),
            params: None,
        };
        let requirements: serde_json::Value = tokio::time::timeout(
            Duration::from_secs(15),
            settings_rpc(client, server_id, session, runtime_kind.clone(), request),
        )
        .await
        .map_err(|_| ClientError::Rpc("Runtime settings policy read timed out".into()))??;
        response["_requirements"] = requirements;
    }
    ensure_settings_session(client, server_id, session)?;
    crate::runtime_settings::snapshot(runtime_kind, response).map_err(ClientError::Serialization)
}

fn convert_params<M, U>(params: M) -> Result<U, ClientError>
where
    M: TryInto<U, Error = crate::RpcClientError>,
{
    params
        .try_into()
        .map_err(|error| ClientError::Serialization(error.to_string()))
}

macro_rules! req {
    ($server_id:expr, $variant:ident, $params:expr) => {
        upstream::ClientRequest::$variant {
            request_id: upstream::RequestId::Integer(next_request_id()),
            params: $params,
        }
    };
}

const MODEL_LIST_RUNTIME_TIMEOUT: Duration = Duration::from_secs(20);
const THREAD_LIST_HYDRATION_BUDGET: usize = 200;

fn thread_list_hydration_budget(params: &types::AppListThreadsRequest) -> Option<usize> {
    let hydrates_recents = params.cursor.is_none()
        && params
            .search_term
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        && !params.use_state_db_only;
    hydrates_recents.then(|| {
        params
            .limit
            .map_or(THREAD_LIST_HYDRATION_BUDGET, |limit| limit as usize)
    })
}

// Only an unfiltered server listing can prove that absent threads were deleted.
fn thread_list_can_prune(params: &types::AppListThreadsRequest) -> bool {
    thread_list_hydration_budget(params).is_some()
        && params.runtime_kinds.as_ref().is_none_or(Vec::is_empty)
        && params.model_providers.as_ref().is_none_or(Vec::is_empty)
        && params.source_kinds.as_ref().is_none_or(Vec::is_empty)
        && params.cwd.is_none()
        && params.archived != Some(true)
}

fn normalize_model_info_for_runtime(
    model_info: &mut types::ModelInfo,
    runtime_kind: types::AgentRuntimeKind,
    visible_modes: Option<&[String]>,
) -> bool {
    let has_qualified_catalog = runtime_kind_uses_qualified_catalog(&runtime_kind);
    model_info.agent_runtime_kind = runtime_kind;
    if has_qualified_catalog && let Some(provider_id) = derive_model_provider_id(&model_info.id) {
        model_info.provider_id = Some(provider_id.to_string());
    }
    // Model IDs, defaults, plugin modes, and effort capabilities belong to the runtime.
    // Litter only classifies entries (model vs mode) and derives picker labels
    // once here so neither platform re-derives them per row.
    model_info.apply_picker_presentation(visible_modes);

    true
}

fn runtime_kind_uses_qualified_catalog(runtime_kind: &str) -> bool {
    matches!(runtime_kind, "pi" | "omp" | "local-studio" | "opencode")
}

fn derive_model_provider_id(id: &str) -> Option<&str> {
    let mut segments = id.split('/');
    let first = segments.next().filter(|segment| !segment.is_empty())?;
    let second = segments.next().filter(|segment| !segment.is_empty())?;
    if first == "openrouter" && segments.next().is_some() {
        return Some(second);
    }
    Some(first)
}

fn runtime_exposes_model_choices(runtime_kind: &str) -> bool {
    runtime_kind != "shell"
}

fn list_runtime_kinds(
    requested: Option<Vec<types::AgentRuntimeKind>>,
    available: &[types::AgentRuntimeKind],
) -> Vec<types::AgentRuntimeKind> {
    let mut runtimes = match requested {
        Some(requested) if !requested.is_empty() => requested
            .into_iter()
            .filter(|kind| available.contains(kind))
            .collect(),
        _ => available.to_vec(),
    };
    runtimes.sort();
    runtimes.dedup();
    runtimes
}

fn apply_thread_goal_to_store(
    client: &MobileClient,
    key: &types::ThreadKey,
    goal: Option<upstream::ThreadGoal>,
) -> Option<types::AppThreadGoal> {
    match goal {
        Some(goal) => {
            let goal = types::AppThreadGoal::from(goal);
            client.app_store.apply_thread_goal(key, goal.clone());
            Some(goal)
        }
        None => {
            client.app_store.clear_thread_goal(key);
            None
        }
    }
}

async fn hydrate_thread_goal_if_available(
    client: &MobileClient,
    server_id: &str,
    key: &types::ThreadKey,
) {
    if client.runtime_for_thread(key) != "codex" {
        return;
    }
    let response: Result<upstream::ThreadGoalGetResponse, ClientError> = rpc_runtime(
        client,
        server_id,
        "codex".to_string(),
        req!(
            server_id,
            ThreadGoalGet,
            upstream::ThreadGoalGetParams {
                thread_id: key.thread_id.clone(),
            }
        ),
    )
    .await;
    match response {
        Ok(response) => {
            apply_thread_goal_to_store(client, key, response.goal);
        }
        Err(error) => {
            tracing::debug!(
                server_id,
                thread_id = key.thread_id,
                %error,
                "thread goal hydration skipped"
            );
        }
    }
}

/// Flatten upstream `plugin/list` marketplaces into a deduped, sorted list of
/// `PluginSummary` rows suitable for `@`-autocomplete. Pure so it can be unit-
/// tested without running an RPC client.
fn shape_plugin_list(response: upstream::PluginListResponse) -> Vec<types::PluginSummary> {
    let mut summaries: Vec<types::PluginSummary> = Vec::new();
    for marketplace in response.marketplaces {
        let marketplace_name = marketplace.name.trim().to_owned();
        if marketplace_name.is_empty() {
            continue;
        }
        let marketplace_path = marketplace.path.map(types::AbsolutePath::from);
        for plugin in marketplace.plugins {
            if plugin.name.trim().is_empty() {
                continue;
            }
            let summary = types::PluginSummary::from_upstream(
                marketplace_name.clone(),
                marketplace_path.clone(),
                plugin,
            );
            if summary.is_available_for_mention() {
                summaries.push(summary);
            }
        }
    }

    // Dedupe by mention_path, keeping the first occurrence.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    summaries.retain(|s| seen.insert(s.mention_path.clone()));

    summaries.sort_by(|a, b| {
        a.display_title
            .to_lowercase()
            .cmp(&b.display_title.to_lowercase())
    });

    summaries
}

fn is_mobile_hidden_skill(skill: &types::SkillMetadata) -> bool {
    if skill.name.trim().eq_ignore_ascii_case("imagegen") {
        return true;
    }

    // Temporary mobile filter: avoid steering image requests into the system
    // imagegen skill while native upstream image_generation is available.
    let mut previous = None;
    for component in skill.path.value.trim_end_matches('/').split('/') {
        if previous == Some(".system") && component.eq_ignore_ascii_case("imagegen") {
            return true;
        }
        previous = Some(component);
    }
    false
}

#[derive(uniffi::Object)]
pub struct AppClient {
    pub(crate) inner: Arc<MobileClient>,
    pub(crate) rt: Arc<tokio::runtime::Runtime>,
}

#[uniffi::export(async_runtime = "tokio")]
impl AppClient {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            inner: shared_mobile_client(),
            rt: shared_runtime(),
        }
    }

    // ── Agent metadata cache ─────────────────────────────────────────────
    //
    // Populated whenever `list_alleycat_agents` succeeds against any
    // server. Platforms read from here to render agent labels, icons,
    // BETA badges, sort order, and capability-driven UI (e.g. Amp's
    // reasoning-effort lock) without baking knowledge of specific
    // agents into Swift / Kotlin.

    /// Look up the cached metadata for a single agent by `name`. Returns
    /// `None` when no probe has populated it yet (cold start) or when
    /// the alleycat host doesn't ship that agent.
    pub fn agent_metadata(&self, name: String) -> Option<crate::store::AppAgentMetadata> {
        self.inner.agent_metadata.get(&name)
    }

    /// All cached agents in presentation-sort order. Empty list on
    /// cold start.
    pub fn all_agent_metadata(&self) -> Vec<crate::store::AppAgentMetadata> {
        self.inner.agent_metadata.all_sorted()
    }

    // ── Thread lifecycle ─────────────────────────────────────────────────

    pub async fn start_thread(
        &self,
        server_id: String,
        params: types::AppStartThreadRequest,
    ) -> Result<types::ThreadKey, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            // New thread: no `thread_id` yet, so no saved apps to
            // reference. `saved_apps_context_for_thread` returns None
            // for an unknown thread; `splice_saved_apps_context` is a
            // no-op in that case, preserving existing behavior.
            let mut params = params;
            let runtime_kind = c.runtime_for_thread_start(
                &server_id,
                params.agent_runtime_kind.clone(),
                params.model.as_deref(),
            );
            crate::widget_guidelines::register_thread_tools(&runtime_kind, &mut params);
            params.developer_instructions = splice_saved_apps_context(
                c.as_ref(),
                &server_id,
                None,
                params.developer_instructions,
            );
            params.developer_instructions =
                splice_generative_ui_preamble(&params.dynamic_tools, params.developer_instructions);
            params.developer_instructions = splice_local_runtime_developer_instructions(
                c.as_ref(),
                &server_id,
                params.developer_instructions,
            );
            let params = convert_params::<_, upstream::ThreadStartParams>(params)?;
            let response: upstream::ThreadStartResponse = rpc_runtime(
                c.as_ref(),
                &server_id,
                runtime_kind.clone(),
                req!(server_id, ThreadStart, params),
            )
            .await?;
            let key = c
                .apply_thread_start_response(&server_id, &response)
                .map_err(ClientError::Serialization)?;
            c.note_thread_runtime(key.clone(), runtime_kind);
            Ok(key)
        })
    }

    pub async fn resume_thread(
        &self,
        server_id: String,
        params: types::AppResumeThreadRequest,
    ) -> Result<types::ThreadKey, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            // Prepend "Apps saved in this thread so far: …" to the
            // developer_instructions so the model knows which slugs are
            // already in use.
            let mut params = params;
            let thread_id = params.thread_id.clone();
            params.developer_instructions = splice_saved_apps_context(
                c.as_ref(),
                &server_id,
                Some(thread_id.as_str()),
                params.developer_instructions,
            );
            params.developer_instructions = splice_local_runtime_developer_instructions(
                c.as_ref(),
                &server_id,
                params.developer_instructions,
            );
            // Resume requests don't carry `dynamic_tools` (the server
            // remembers them from start). The preamble was injected at
            // start_thread; developer_instructions persist server-side
            // across turns, so no re-injection needed here.
            let params = convert_params::<_, upstream::ThreadResumeParams>(params)?;
            let response: upstream::ThreadResumeResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadResume, params),
            )
            .await?;
            let key = c
                .apply_thread_resume_response(&server_id, &response)
                .map_err(ClientError::Serialization)?;
            hydrate_thread_goal_if_available(c.as_ref(), &server_id, &key).await;
            Ok(key)
        })
    }

    /// Register the directory where `saved_apps.rs` persists the app
    /// index + per-app files. Platforms (iOS/Android) call this once at
    /// process start with the same path they pass to `saved_apps_list`.
    /// When set, the `show_widget` auto-upsert hook in the dynamic-tool
    /// handler uses this directory to persist finalized widgets.
    pub fn set_saved_apps_directory(&self, directory: String) {
        let mut guard = self
            .inner
            .saved_apps_directory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = if directory.is_empty() {
            None
        } else {
            Some(directory)
        };
    }

    /// Register the directory where Slingshot controller enrollment
    /// credentials are persisted. Platforms should pass their private
    /// preferences/application-support directory at launch.
    pub fn set_slingshot_credentials_directory(&self, directory: String) {
        let mut guard = self
            .inner
            .slingshot_credentials_directory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = if directory.is_empty() {
            None
        } else {
            Some(directory)
        };
    }

    /// Pre-load the persisted iroh device secret key bytes (32 bytes).
    /// Call from the platform at app launch BEFORE any alleycat
    /// operation so the shared `Endpoint` reuses the same `EndpointId`
    /// across cold launches. Pass `None` (or an empty/invalid-length
    /// vector) to clear and force fresh-key generation on next bind.
    pub fn set_alleycat_secret_key(&self, secret_key_bytes: Option<Vec<u8>>) {
        self.inner.set_alleycat_secret_key(secret_key_bytes);
    }

    /// Read back the secret key bytes the alleycat endpoint is bound to,
    /// or `None` if the endpoint hasn't been initialized yet. Platform
    /// calls this AFTER the first alleycat operation has triggered a
    /// bind, then persists the bytes to keychain /
    /// EncryptedSharedPreferences so subsequent launches reuse them.
    pub fn alleycat_secret_key(&self) -> Option<Vec<u8>> {
        self.inner.alleycat_secret_key()
    }

    /// Force the shared alleycat iroh `Endpoint` to bind now (instead
    /// of lazily on first use). Returns the secret key bytes the
    /// endpoint was bound with — either the bytes the platform pushed
    /// via `set_alleycat_secret_key` or freshly-generated bytes the
    /// platform should now persist. Returns `None` if the bind itself
    /// fails (e.g. UDP socket unavailable). Idempotent: subsequent
    /// calls return the same bytes from the already-bound endpoint.
    ///
    /// Use this from the platform when you've already loaded a
    /// persisted key from secure storage and want a deterministic
    /// "load → bind → save" sequence at launch (avoids racing the
    /// lazy-init path against the lifecycle reconnect chain).
    pub async fn ensure_alleycat_endpoint(&self) -> Option<Vec<u8>> {
        match self.inner.alleycat_endpoint().await {
            Ok(endpoint) => Some(endpoint.secret_key().to_bytes().to_vec()),
            Err(error) => {
                tracing::warn!("ensure_alleycat_endpoint: bind failed: {error}");
                None
            }
        }
    }

    /// Gracefully close the shared alleycat iroh `Endpoint`. Call from
    /// the platform on app termination so iroh sends a clean
    /// CONNECTION_CLOSE to peers instead of logging "Aborting
    /// ungracefully" when the static `MobileClient` slot is finally
    /// dropped at process exit. Best-effort: iOS doesn't always fire
    /// `applicationWillTerminate` reliably, so this hook may be
    /// unreachable on swipe-up-to-kill — that's acceptable, the cost
    /// is one log line on iroh's side and the daemon waiting up to
    /// `max_idle_timeout` to reap that one final zombie.
    pub async fn shutdown_alleycat_endpoint(&self) {
        self.inner.shutdown_alleycat_endpoint().await;
    }

    /// Register the directory where Rust can persist small local app
    /// preferences that are not part of the public preferences record.
    pub fn set_mobile_preferences_directory(&self, directory: String) {
        self.inner.set_mobile_preferences_directory(directory);
    }

    pub async fn fork_thread(
        &self,
        server_id: String,
        params: types::AppForkThreadRequest,
    ) -> Result<types::ThreadKey, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let mut params = params;
            params.developer_instructions = splice_local_runtime_developer_instructions(
                c.as_ref(),
                &server_id,
                params.developer_instructions,
            );
            let params = convert_params::<_, upstream::ThreadForkParams>(params)?;
            let response: upstream::ThreadForkResponse =
                rpc(c.as_ref(), &server_id, req!(server_id, ThreadFork, params)).await?;
            c.apply_thread_fork_response(&server_id, &response)
                .map_err(ClientError::Serialization)
        })
    }

    pub async fn archive_thread(
        &self,
        server_id: String,
        params: types::AppArchiveThreadRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::ThreadArchiveResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadArchive, params.into()),
            )
            .await?;
            Ok(())
        })
    }

    pub async fn rename_thread(
        &self,
        server_id: String,
        params: types::AppRenameThreadRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::ThreadSetNameResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadSetName, params.into()),
            )
            .await?;
            Ok(())
        })
    }

    pub async fn get_thread_goal(
        &self,
        server_id: String,
        params: types::AppThreadGoalGetRequest,
    ) -> Result<Option<types::AppThreadGoal>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let thread_id = params.thread_id.clone();
            let response: upstream::ThreadGoalGetResponse = rpc_runtime(
                c.as_ref(),
                &server_id,
                "codex".to_string(),
                req!(server_id, ThreadGoalGet, params.into()),
            )
            .await?;
            let key = types::ThreadKey {
                server_id,
                thread_id,
            };
            Ok(apply_thread_goal_to_store(c.as_ref(), &key, response.goal))
        })
    }

    pub async fn set_thread_goal(
        &self,
        server_id: String,
        params: types::AppThreadGoalSetRequest,
    ) -> Result<types::AppThreadGoal, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let thread_id = params.thread_id.clone();
            let response: upstream::ThreadGoalSetResponse = rpc_runtime(
                c.as_ref(),
                &server_id,
                "codex".to_string(),
                req!(server_id, ThreadGoalSet, params.into()),
            )
            .await?;
            let key = types::ThreadKey {
                server_id,
                thread_id,
            };
            let goal = types::AppThreadGoal::from(response.goal);
            c.app_store.apply_thread_goal(&key, goal.clone());
            Ok(goal)
        })
    }

    pub async fn clear_thread_goal(
        &self,
        server_id: String,
        params: types::AppThreadGoalClearRequest,
    ) -> Result<bool, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let thread_id = params.thread_id.clone();
            let response: upstream::ThreadGoalClearResponse = rpc_runtime(
                c.as_ref(),
                &server_id,
                "codex".to_string(),
                req!(server_id, ThreadGoalClear, params.into()),
            )
            .await?;
            if response.cleared {
                let key = types::ThreadKey {
                    server_id,
                    thread_id,
                };
                c.app_store.clear_thread_goal(&key);
            }
            Ok(response.cleared)
        })
    }

    pub async fn list_threads(
        &self,
        server_id: String,
        params: types::AppListThreadsRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let requested_runtime_kinds = params.runtime_kinds.clone();
            let hydration_budget = thread_list_hydration_budget(&params);
            let can_prune = thread_list_can_prune(&params);
            let params: upstream::ThreadListParams = params.into();
            let session = c
                .get_session(&server_id)
                .map_err(|error| ClientError::Rpc(error.to_string()))?;
            let available_runtime_kinds = session.runtime_kinds();
            tracing::info!(
                "list_threads: resolve runtimes server_id={} requested={:?} available={:?} search_term={:?} use_state_db_only={} limit={:?} cursor={:?}",
                server_id,
                requested_runtime_kinds,
                available_runtime_kinds,
                params.search_term,
                params.use_state_db_only,
                params.limit,
                params.cursor
            );
            let runtime_kinds =
                list_runtime_kinds(requested_runtime_kinds, &available_runtime_kinds);
            if runtime_kinds.is_empty() {
                return Err(ClientError::Rpc(
                    "none of the requested agent runtimes are available on this controller"
                        .to_string(),
                ));
            }
            tracing::info!(
                "list_threads: fanout start server_id={} runtime_kinds={:?}",
                server_id,
                runtime_kinds
            );

            // Fan out per-runtime concurrently. The previous sequential loop
            // exhausted Codex's full cursor pagination before non-Codex runtimes
            // ever got their first page, so a Codex inbox with many threads
            // would starve the other providers — the user would see only
            // Codex threads while other runtimes were silently waiting their
            // turn. By spawning each runtime's pagination as its own future
            // and joining them, every provider's first page lands in
            // parallel and the UI gets representative threads from each
            // immediately.
            let mut codex_visited = false;
            let mut tasks = Vec::new();
            for runtime_kind in runtime_kinds {
                if runtime_kind == "codex" {
                    if codex_visited {
                        continue;
                    }
                    codex_visited = true;
                }

                let client = std::sync::Arc::clone(c);
                let server_id = server_id.clone();
                let initial_params = params.clone();
                tasks.push(async move {
                    let mut request_params = initial_params;
                    let mut ids = Vec::new();
                    let mut completed = true;
                    let mut exhausted = false;
                    loop {
                        let response: upstream::ThreadListResponse =
                            match rpc_runtime::<upstream::ThreadListResponse>(
                                client.as_ref(),
                                &server_id,
                                runtime_kind.clone(),
                                req!(server_id, ThreadList, request_params.clone()),
                            )
                            .await
                            {
                                Ok(response) => response,
                                Err(error) => {
                                    tracing::warn!(
                                        "list_threads: thread/list failed for runtime {:?} on server {}: {}",
                                        runtime_kind, server_id, error
                                    );
                                    completed = false;
                                    break;
                                }
                            };
                        let page_was_empty = response.data.is_empty();
                        let page = client.upsert_thread_list_page_for_runtime(
                            &server_id,
                            runtime_kind.clone(),
                            &response.data,
                        );
                        ids.extend(page.into_iter().map(|thread| thread.id));
                        let Some(next_cursor) = response.next_cursor else {
                            exhausted = true;
                            break;
                        };
                        let Some(budget) = hydration_budget else {
                            break;
                        };
                        if page_was_empty || ids.len() >= budget {
                            break;
                        }
                        request_params.cursor = Some(next_cursor);
                    }
                    (runtime_kind, ids, completed, exhausted)
                });
            }

            let results = futures::future::join_all(tasks).await;
            if results.iter().all(|(_, _, completed, _)| !completed) {
                return Err(ClientError::Rpc(
                    "thread list failed for every runtime".into(),
                ));
            }
            // Only prune if every runtime finished cleanly. A partial
            // result (one runtime timed out / errored) means we don't
            // know its true thread set yet, and `finalize_thread_list_sync`
            // would delete unseen threads from healthy runtimes too —
            // wiping pi/opencode threads from the store on a transient
            // codex failure. Skip pruning in that case; the next refresh
            // reconciles when the failing runtime recovers.
            let all_completed = results.iter().all(|(_, _, ok, _)| *ok);
            let all_exhausted = results.iter().all(|(_, _, _, exhausted)| *exhausted);
            if all_completed && all_exhausted && can_prune {
                let mut all_thread_ids = Vec::new();
                for (_, ids, _, _) in results {
                    all_thread_ids.extend(ids);
                }
                c.finalize_thread_list_sync(&server_id, all_thread_ids);
            } else if !all_completed {
                tracing::warn!(
                    "list_threads: skipping finalize prune — partial fan-out result on server {}",
                    server_id
                );
            }
            Ok(())
        })
    }

    pub async fn read_thread(
        &self,
        server_id: String,
        params: types::AppReadThreadRequest,
    ) -> Result<types::ThreadKey, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::ThreadReadResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadRead, params.into()),
            )
            .await?;
            let key = c
                .apply_thread_read_response(&server_id, &response)
                .map_err(ClientError::Serialization)?;
            hydrate_thread_goal_if_available(c.as_ref(), &server_id, &key).await;
            Ok(key)
        })
    }

    // ── Turn ─────────────────────────────────────────────────────────────

    pub async fn interrupt_turn(
        &self,
        server_id: String,
        params: types::AppInterruptTurnRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let thread_id = params.thread_id.clone();
            let turn_id = params.turn_id.clone();
            let _: upstream::TurnInterruptResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, TurnInterrupt, params.into()),
            )
            .await?;
            c.mark_turn_interrupted_locally(&server_id, &thread_id, &turn_id);
            Ok(())
        })
    }

    pub async fn list_collaboration_modes(
        &self,
        server_id: String,
    ) -> Result<Vec<types::AppCollaborationModePreset>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            c.server_collaboration_mode_list(&server_id)
                .await
                .map_err(|e| ClientError::Rpc(e.to_string()))
        })
    }

    // ── Realtime / voice ─────────────────────────────────────────────────

    pub async fn start_realtime_session(
        &self,
        server_id: String,
        params: types::AppStartRealtimeSessionRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let thread_id = params.thread_id.clone();
            let params = convert_params::<_, upstream::ThreadRealtimeStartParams>(params)?;
            let response: Result<upstream::ThreadRealtimeStartResponse, ClientError> = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadRealtimeStart, params.clone()),
            )
            .await;
            if let Err(error) = response {
                if !is_stale_thread_error(&error.to_string()) {
                    return Err(error);
                }
                c.force_refresh_thread_authoritative(&server_id, &thread_id)
                    .await
                    .map_err(|recover_error| ClientError::Rpc(recover_error.to_string()))?;
                let _: upstream::ThreadRealtimeStartResponse = rpc(
                    c.as_ref(),
                    &server_id,
                    req!(server_id, ThreadRealtimeStart, params),
                )
                .await?;
            }
            Ok(())
        })
    }

    pub async fn stop_realtime_session(
        &self,
        server_id: String,
        params: types::AppStopRealtimeSessionRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::ThreadRealtimeStopResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadRealtimeStop, params.into()),
            )
            .await?;
            Ok(())
        })
    }

    pub async fn resolve_realtime_handoff(
        &self,
        server_id: String,
        params: types::AppResolveRealtimeHandoffRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::ThreadRealtimeResolveHandoffResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadRealtimeResolveHandoff, params.into()),
            )
            .await?;
            Ok(())
        })
    }

    pub async fn finalize_realtime_handoff(
        &self,
        server_id: String,
        params: types::AppFinalizeRealtimeHandoffRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::ThreadRealtimeFinalizeHandoffResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ThreadRealtimeFinalizeHandoff, params.into()),
            )
            .await?;
            Ok(())
        })
    }

    // ── Review ───────────────────────────────────────────────────────────

    pub async fn start_review(
        &self,
        server_id: String,
        params: types::AppStartReviewRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let params = convert_params::<_, upstream::ReviewStartParams>(params)?;
            let _: upstream::ReviewStartResponse =
                rpc(c.as_ref(), &server_id, req!(server_id, ReviewStart, params)).await?;
            Ok(())
        })
    }

    // ── Models & features ────────────────────────────────────────────────

    /// Read a provider-specific catalog without replacing the default model cache.
    /// Older servers can reject the selector; do not silently return another provider.
    pub async fn list_models_for_provider(
        &self,
        server_id: String,
        provider_id: String,
    ) -> Result<Vec<types::ModelInfo>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let provider_id = provider_id.trim();
            if provider_id.is_empty() {
                return Err(ClientError::Rpc(
                    "Choose a configured model provider".into(),
                ));
            }
            let session = c
                .get_session(&server_id)
                .map_err(|e| ClientError::Rpc(e.to_string()))?;
            // Older servers may silently ignore unknown model/list fields. Probe
            // provider discovery before trusting the provider-specific response.
            let mut health = session.health();
            health.borrow_and_update();
            let check_connection = || -> Result<(), ClientError> {
                ensure_settings_session(c, &server_id, &session)?;
                if health.has_changed().unwrap_or(true) {
                    return Err(ClientError::Rpc("Connection changed while querying provider models; retry the catalog".into()));
                }
                Ok(())
            };
            let providers: upstream::ModelProviderListResponse = tokio::time::timeout(
                MODEL_LIST_RUNTIME_TIMEOUT,
                rpc_runtime(
                    c,
                    &server_id,
                    "codex".into(),
                    req!(
                        server_id,
                        ModelProviderList,
                        upstream::ModelProviderListParams {}
                    ),
                ),
            )
            .await
            .map_err(|_| ClientError::Rpc("Provider discovery timed out".into()))??;
            check_connection()?;
            if !providers
                .data
                .iter()
                .any(|provider| provider.id == provider_id)
            {
                return Err(ClientError::Rpc(
                    "Model provider is not configured on this server".into(),
                ));
            }
            // Discovery predates the model/list selector. A read against a
            // guaranteed-unknown provider must be rejected with its typed error;
            // an old peer silently accepting this probe cannot select providers.
            let probe = loop {
                let id = format!("litter-provider-capability-probe-{}", next_request_id());
                if !providers.data.iter().any(|provider| provider.id == id) { break id; }
            };
            let probe_request = req!(server_id, ModelList, upstream::ModelListParams {
                model_provider: Some(probe.clone()), cursor: None, limit: Some(1), include_hidden: None,
            });
            let probe_result = tokio::time::timeout(MODEL_LIST_RUNTIME_TIMEOUT,
                session.request_client_for_runtime("codex".into(), probe_request)).await
                .map_err(|_| ClientError::Rpc("Provider capability check timed out".into()))?;
            let supported = matches!(&probe_result, Err(crate::transport::RpcError::ServerWithData {data, ..})
                if matches!(data.get("type").and_then(|v| v.as_str()), Some("modelProviderNotConfigured" | "modelProviderPolicyConflict"))
                    && data.get("modelProvider").and_then(|v| v.as_str()) == Some(probe.as_str()));
            check_connection()?;
            if !supported {
                return Err(ClientError::Rpc("This server cannot verify provider-specific model queries; update Codex on the connected computer".into()));
            }
            let mut params = upstream::ModelListParams {
                model_provider: Some(provider_id.to_string()),
                cursor: None,
                limit: Some(100),
                include_hidden: Some(true),
            };
            let mut models = Vec::new();
            let mut cursors = HashSet::new();
            loop {
                let page: upstream::ModelListResponse = tokio::time::timeout(
                    MODEL_LIST_RUNTIME_TIMEOUT,
                    rpc_runtime(
                        c,
                        &server_id,
                        "codex".into(),
                        req!(server_id, ModelList, params.clone()),
                    ),
                )
                .await
                .map_err(|_| ClientError::Rpc("Provider model catalog timed out".into()))??;
                check_connection()?;
                models.extend(page.data.into_iter().map(|value| {
                    let mut model = types::ModelInfo::from(value);
                    model.provider_id = Some(provider_id.to_string());
                    model.apply_picker_presentation(None);
                    model
                }));
                let Some(cursor) = page.next_cursor else {
                    break;
                };
                if !cursors.insert(cursor.clone()) {
                    return Err(ClientError::Rpc(
                        "Provider model catalog repeated a cursor".into(),
                    ));
                }
                params.cursor = Some(cursor);
            }
            Ok(models)
        })
    }

    pub fn models_need_refresh(&self, server_id: String) -> bool {
        self.inner.models_need_refresh(&server_id)
    }

    pub async fn refresh_models(
        &self,
        server_id: String,
        params: types::AppRefreshModelsRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            use futures::StreamExt;

            // Forced refreshes after settings writes must fetch after the older
            // catalog completes, rather than be dropped or race its publication.
            let catalog_lock = c.model_catalog_lock(&server_id);
            let _catalog_guard = catalog_lock.lock().await;
            let session = c
                .get_session(&server_id)
                .map_err(|error| ClientError::Rpc(error.to_string()))?;
            let catalog_runtimes = session.runtime_kinds();
            let mut runtime_kinds = catalog_runtimes
                .clone()
                .into_iter()
                .filter(|runtime_kind| runtime_exposes_model_choices(runtime_kind))
                .collect::<Vec<_>>();
            runtime_kinds.sort();
            runtime_kinds.dedup();
            let params: upstream::ModelListParams = params.into();
            let requested_runtimes = runtime_kinds.clone();
            let mut tasks = runtime_kinds
                .into_iter()
                .map(|runtime_kind| {
                    let client = Arc::clone(c);
                    let server_id = server_id.clone();
                    let mut request_params = params.clone();
                    let visible_modes = client
                        .agent_metadata
                        .get(&runtime_kind)
                        .and_then(|metadata| metadata.capabilities)
                        .and_then(|capabilities| capabilities.visible_modes);
                    async move {
                        let fetch = async {
                            let mut models = Vec::new();
                            let mut cursors = HashSet::new();
                            if let Some(cursor) = &request_params.cursor {
                                cursors.insert(cursor.clone());
                            }
                            loop {
                                let page: upstream::ModelListResponse = rpc_runtime(
                                    client.as_ref(),
                                    &server_id,
                                    runtime_kind.clone(),
                                    req!(server_id, ModelList, request_params.clone()),
                                )
                                .await?;
                                models.extend(page.data.into_iter().filter_map(|model| {
                                    let mut model = types::ModelInfo::from(model);
                                    normalize_model_info_for_runtime(
                                        &mut model,
                                        runtime_kind.clone(),
                                        visible_modes.as_deref(),
                                    )
                                    .then_some(model)
                                }));
                                let Some(cursor) = page.next_cursor else {
                                    break;
                                };
                                if !cursors.insert(cursor.clone()) {
                                    return Err(ClientError::Rpc(format!(
                                        "{runtime_kind}: model/list repeated a cursor"
                                    )));
                                }
                                request_params.cursor = Some(cursor);
                            }
                            Ok::<_, ClientError>(models)
                        };
                        let started = std::time::Instant::now();
                        let result = tokio::time::timeout(MODEL_LIST_RUNTIME_TIMEOUT, fetch).await;
                        tracing::info!(
                            operation = "model_list",
                            runtime_kind = %runtime_kind,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            success = matches!(&result, Ok(Ok(_))),
                            "mobile request timing"
                        );
                        (runtime_kind, result)
                    }
                })
                .collect::<futures::stream::FuturesUnordered<_>>();
            let mut failures = Vec::new();
            while let Some((runtime_kind, result)) = tasks.next().await {
                match result {
                    Ok(Ok(runtime_models)) => {
                        c.publish_model_catalog_runtime(
                            &server_id,
                            &session,
                            &requested_runtimes,
                            Some((&runtime_kind, runtime_models)),
                        )
                        .map_err(ClientError::Rpc)?;
                    }
                    Ok(Err(error)) => {
                        failures.push(format!("{runtime_kind}: {error}"));
                    }
                    Err(_) => {
                        failures.push(format!("{runtime_kind}: model/list timed out"));
                    }
                }
            }
            // Also prune removed runtimes when every request failed or no runtime
            // exposes model choices. Failed and pending runtimes retain their cache.
            c.publish_model_catalog_runtime(&server_id, &session, &requested_runtimes, None)
                .map_err(ClientError::Rpc)?;
            if !c.note_model_catalog_refresh(
                &server_id,
                &session,
                catalog_runtimes,
                failures.is_empty(),
            ) {
                return Err(ClientError::Rpc(
                    "Connection changed while loading models; retry the catalog".into(),
                ));
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(ClientError::Rpc(failures.join("; ")))
            }
        })
    }

    pub async fn list_experimental_features(
        &self,
        server_id: String,
        params: types::AppListExperimentalFeaturesRequest,
    ) -> Result<Vec<types::ExperimentalFeature>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::ExperimentalFeatureListResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ExperimentalFeatureList, params.into()),
            )
            .await?;
            Ok(response.data.into_iter().map(Into::into).collect())
        })
    }

    pub async fn list_skills(
        &self,
        server_id: String,
        params: types::AppListSkillsRequest,
    ) -> Result<Vec<types::SkillMetadata>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::SkillsListResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, SkillsList, params.into()),
            )
            .await?;
            Ok(response
                .data
                .into_iter()
                .flat_map(|entry| entry.skills.into_iter().map(Into::into))
                .filter(|skill| !is_mobile_hidden_skill(skill))
                .collect())
        })
    }

    pub async fn list_plugins(
        &self,
        server_id: String,
        params: types::AppListPluginsRequest,
    ) -> Result<Vec<types::PluginSummary>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let upstream_params = convert_params::<_, upstream::PluginListParams>(params)?;
            let response: upstream::PluginListResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, PluginList, upstream_params),
            )
            .await?;
            Ok(shape_plugin_list(response))
        })
    }

    // ── Account ──────────────────────────────────────────────────────────

    pub async fn login_account(
        &self,
        server_id: String,
        params: types::AppLoginAccountRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::LoginAccountResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, LoginAccount, params.into()),
            )
            .await?;
            c.sync_server_account(&server_id)
                .await
                .map_err(|error| ClientError::Rpc(error.to_string()))
        })
    }

    pub async fn logout_account(&self, server_id: String) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let _: upstream::LogoutAccountResponse =
                rpc(c.as_ref(), &server_id, req!(server_id, LogoutAccount, None)).await?;
            c.sync_server_account_after_logout(&server_id)
                .await
                .map_err(|error| ClientError::Rpc(error.to_string()))
        })
    }

    pub async fn refresh_rate_limits(&self, server_id: String) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::GetAccountRateLimitsResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, GetAccountRateLimits, None),
            )
            .await?;
            c.apply_account_rate_limits_response(&server_id, "codex".to_string(), &response);
            Ok(())
        })
    }

    pub async fn refresh_account(
        &self,
        server_id: String,
        params: types::AppRefreshAccountRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::GetAccountResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, GetAccount, params.into()),
            )
            .await?;
            c.apply_account_response(&server_id, &response);
            Ok(())
        })
    }

    pub async fn auth_status(
        &self,
        server_id: String,
        params: types::AuthStatusRequest,
    ) -> Result<types::AuthStatus, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: upstream::GetAuthStatusResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, GetAuthStatus, params.into()),
            )
            .await?;
            Ok(response.into())
        })
    }

    // ── Utilities ────────────────────────────────────────────────────────

    pub async fn resolve_image_view(
        &self,
        server_id: String,
        path: String,
    ) -> Result<types::ResolvedImageViewResult, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            resolve_image_view_bytes(c.as_ref(), &server_id, &path, None).await
        })
    }

    /// Resolve an image path relative to a thread working directory when the
    /// message referenced `./foo.png`, `art/foo.svg`, or another relative
    /// path. The original `resolve_image_view` remains available for
    /// absolute/tool-provided paths.
    pub async fn resolve_image_view_at(
        &self,
        server_id: String,
        path: String,
        cwd: Option<String>,
    ) -> Result<types::ResolvedImageViewResult, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            resolve_image_view_bytes(c.as_ref(), &server_id, &path, cwd.as_deref()).await
        })
    }

    pub async fn list_pets(
        &self,
        server_id: String,
    ) -> Result<Vec<types::AppPetSummary>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let entries = scan_remote_pets(c.as_ref(), &server_id).await?;
            Ok(entries.into_iter().map(|entry| entry.summary).collect())
        })
    }

    pub async fn load_pet(
        &self,
        server_id: String,
        pet_id: String,
    ) -> Result<types::AppPetPackage, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let trimmed_pet_id = pet_id.trim();
            if trimmed_pet_id.is_empty() {
                return Err(ClientError::InvalidParams("pet id is empty".to_string()));
            }
            let entries = scan_remote_pets(c.as_ref(), &server_id).await?;
            let entry = entries
                .into_iter()
                .find(|entry| entry.summary.id == trimmed_pet_id)
                .ok_or_else(|| ClientError::Rpc(format!("pet not found: {trimmed_pet_id}")))?;
            if let Some(error) = entry.summary.validation_error.as_ref() {
                return Err(ClientError::Rpc(error.clone()));
            }
            let spritesheet_file = entry
                .summary
                .spritesheet_path
                .as_deref()
                .ok_or_else(|| ClientError::Rpc("pet has no spritesheet path".to_string()))?;
            let spritesheet_path =
                crate::pets::local_spritesheet_path(&entry.summary.source_path, spritesheet_file)
                    .map_err(ClientError::Serialization)?;
            let spritesheet_bytes =
                read_remote_file_bytes(c.as_ref(), &server_id, &spritesheet_path).await?;
            crate::pets::package_from_parts(
                entry.summary.source_path,
                &entry.manifest_json,
                spritesheet_bytes,
            )
            .map_err(ClientError::Serialization)
        })
    }

    pub async fn runtime_settings(
        &self,
        server_id: String,
        runtime_kind: types::AgentRuntimeKind,
    ) -> Result<crate::runtime_settings::RuntimeSettingsSnapshot, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let session = c
                .get_session(&server_id)
                .map_err(|e| ClientError::Rpc(e.to_string()))?;
            read_runtime_settings(c.as_ref(), &server_id, &session, runtime_kind).await
        })
    }
    pub async fn set_runtime_setting(
        &self,
        server_id: String,
        runtime_kind: types::AgentRuntimeKind,
        key: String,
        value_json: String,
    ) -> Result<crate::runtime_settings::RuntimeSettingsSnapshot, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let session = c
                .get_session(&server_id)
                .map_err(|e| ClientError::Rpc(e.to_string()))?;
            let value: serde_json::Value = serde_json::from_str(&value_json)
                .map_err(|e| ClientError::Serialization(e.to_string()))?;
            crate::runtime_settings::validate_edit(&key, &value)
                .map_err(ClientError::Serialization)?;
            let before =
                read_runtime_settings(c.as_ref(), &server_id, &session, runtime_kind.clone())
                    .await?;
            let descriptor = before
                .settings
                .iter()
                .find(|s| s.key == key && s.writable)
                .ok_or_else(|| {
                    ClientError::Rpc("Setting is not advertised as writable by this runtime".into())
                })?;
            if !descriptor.choices.is_empty()
                && !value
                    .as_str()
                    .is_some_and(|v| descriptor.choices.iter().any(|choice| choice == v))
            {
                return Err(ClientError::Rpc(
                    "Setting value is outside the runtime's allowed choices".into(),
                ));
            }
            let params = serde_json::from_value(
                serde_json::json!({"keyPath":key,"value":value,"mergeStrategy":"replace"}),
            )
            .map_err(|e| ClientError::Serialization(e.to_string()))?;
            let _: serde_json::Value = tokio::time::timeout(
                Duration::from_secs(15),
                settings_rpc(
                    c.as_ref(),
                    &server_id,
                    &session,
                    runtime_kind.clone(),
                    req!(server_id, ConfigValueWrite, params),
                ),
            )
            .await
            .map_err(|_| {
                ClientError::Rpc(
                    "Settings write timed out; refresh to verify whether it was applied".into(),
                )
            })??;
            let after =
                read_runtime_settings(c.as_ref(), &server_id, &session, runtime_kind).await?;
            if !after.settings.iter().any(|s| {
                s.key == key
                    && serde_json::from_str::<serde_json::Value>(&s.value_json)
                        .ok()
                        .as_ref()
                        == Some(&value)
            }) {
                return Err(ClientError::Rpc("Runtime settings read-back differs from the requested value (a policy or project setting may override it)".into()));
            }
            Ok(after)
        })
    }

    pub async fn write_config_value(
        &self,
        server_id: String,
        params: types::AppWriteConfigValueRequest,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let params = convert_params::<_, upstream::ConfigValueWriteParams>(params)?;
            let _: upstream::ConfigWriteResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, ConfigValueWrite, params),
            )
            .await?;
            Ok(())
        })
    }

    pub async fn search_files(
        &self,
        server_id: String,
        params: types::AppSearchFilesRequest,
    ) -> Result<Vec<types::FileSearchResult>, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let response: WireFuzzyFileSearchResponse = rpc(
                c.as_ref(),
                &server_id,
                req!(server_id, FuzzyFileSearch, params.into()),
            )
            .await?;
            Ok(response.files.into_iter().map(Into::into).collect())
        })
    }

    pub async fn start_remote_ssh_oauth_login(
        &self,
        server_id: String,
    ) -> Result<String, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            c.start_remote_ssh_oauth_login(&server_id)
                .await
                .map_err(|error| ClientError::Rpc(error.to_string()))
        })
    }

    // ── Directory browsing ──────────────────────────────────────────────

    /// Resolve the home directory on a remote server.
    ///
    /// Tries POSIX `$HOME` first, falls back to Windows `%USERPROFILE%`.
    /// Returns `"/"` if both fail.
    pub async fn resolve_remote_home(&self, server_id: String) -> Result<String, ClientError> {
        if let Some(home) = crate::remote_dir_cache::home(&server_id) {
            return Ok(home);
        }
        blocking_async!(self.rt, self.inner, |c| {
            // Try POSIX. `$HOME` is set without a login shell; `-l` would
            // run the user's whole profile on every picker open.
            if let Ok(resp) = exec_command_simple(
                c.as_ref(),
                &server_id,
                &["/usr/bin/env", "sh", "-c", r#"printf %s "$HOME""#],
                Some("/tmp"),
            )
            .await
                && resp.exit_code == 0
            {
                let home = resp.stdout.trim().to_string();
                if !home.is_empty() {
                    crate::remote_dir_cache::store_home(&server_id, &home);
                    return Ok(home);
                }
            }
            // Fallback: Windows
            if let Ok(resp) = exec_command_simple(
                c.as_ref(),
                &server_id,
                &["cmd.exe", "/c", "echo", "%USERPROFILE%"],
                None,
            )
            .await
                && resp.exit_code == 0
            {
                let home = resp.stdout.trim().to_string();
                if !home.is_empty() && home != "%USERPROFILE%" {
                    crate::remote_dir_cache::store_home(&server_id, &home);
                    return Ok(home);
                }
            }
            Ok("/".to_string())
        })
    }

    /// List subdirectories in a remote directory.
    ///
    /// Auto-detects Windows vs POSIX from the path format and runs the
    /// appropriate command. Returns sorted directory names.
    pub async fn list_remote_directory(
        &self,
        server_id: String,
        path: String,
    ) -> Result<types::DirectoryListResult, ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let requested_path = {
                let p = path.trim();
                if p.is_empty() {
                    "/".to_string()
                } else {
                    crate::remote_path::normalize_thread_cwd(p).unwrap_or_else(|| p.to_string())
                }
            };
            let rp = crate::remote_path::RemotePath::parse(&requested_path);
            let normalized = rp.as_str().to_string();
            let is_windows = rp.is_windows();

            if let Some(directories) = crate::remote_dir_cache::listing(&server_id, &normalized) {
                return Ok(types::DirectoryListResult {
                    directories,
                    path: normalized,
                });
            }

            let (command, cwd): (Vec<&str>, &str) = if is_windows {
                // `dir /b /ad` in cwd — avoids path quoting issues
                (vec!["cmd.exe", "/c", "dir", "/b", "/ad"], &normalized)
            } else {
                (vec!["/usr/bin/env", "ls", "-1ap", &normalized], &normalized)
            };

            let resp = exec_command_simple(c.as_ref(), &server_id, &command, Some(cwd)).await?;

            if resp.exit_code != 0 {
                let msg = resp.stderr.trim();
                return Err(ClientError::Rpc(if msg.is_empty() {
                    format!("listing failed with exit code {}", resp.exit_code)
                } else {
                    msg.to_string()
                }));
            }

            let directories = crate::remote_path::parse_directory_listing(&resp.stdout, is_windows);
            crate::remote_dir_cache::store_listing(&server_id, &normalized, &directories);
            Ok(types::DirectoryListResult {
                directories,
                path: normalized,
            })
        })
    }

    /// Create a directory on a remote server. Creates intermediate parents
    /// as needed. No-op (returns Ok) if the directory already exists.
    pub async fn create_remote_directory(
        &self,
        server_id: String,
        path: String,
    ) -> Result<(), ClientError> {
        blocking_async!(self.rt, self.inner, |c| {
            let requested_path = crate::remote_path::normalize_thread_cwd(path.trim())
                .unwrap_or_else(|| path.trim().to_string());
            if requested_path.is_empty() {
                return Err(ClientError::Rpc("path is empty".to_string()));
            }
            let rp = crate::remote_path::RemotePath::parse(&requested_path);
            let normalized = rp.as_str().to_string();
            let is_windows = rp.is_windows();

            // `mkdir -p` on POSIX is idempotent. On Windows we fall back to
            // PowerShell `New-Item -Force` which mirrors that behavior and
            // handles intermediate parents.
            let command: Vec<&str> = if is_windows {
                vec![
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    "New-Item",
                    "-ItemType",
                    "Directory",
                    "-Force",
                    "-Path",
                    &normalized,
                ]
            } else {
                vec!["/usr/bin/env", "mkdir", "-p", &normalized]
            };

            let resp = exec_command_simple(c.as_ref(), &server_id, &command, None).await?;
            if resp.exit_code != 0 {
                let msg = resp.stderr.trim();
                return Err(ClientError::Rpc(if msg.is_empty() {
                    format!("mkdir failed with exit code {}", resp.exit_code)
                } else {
                    msg.to_string()
                }));
            }
            crate::remote_dir_cache::invalidate_listings(&server_id);
            Ok(())
        })
    }

    // ── Local codex (direct-dist Mac) ────────────────────────────────────

    /// Attach to an existing `codex app-server` on `127.0.0.1:{port}`, or
    /// spawn one using the bundled binary resolver. The returned record
    /// carries enough data to drive `connect_remote_server` on the Swift
    /// side; if `handle` is present the caller must hold it (and call
    /// `stop()` on app termination) to keep the child alive.
    pub async fn attach_or_spawn_local_server(
        &self,
        port: u16,
        codex_home: Option<String>,
    ) -> Result<LocalServerConnection, ClientError> {
        let rt = Arc::clone(&self.rt);
        let codex_home = codex_home.map(std::path::PathBuf::from);
        let attach = rt
            .spawn(async move {
                crate::local_server::attach_or_spawn_local_server(port, codex_home).await
            })
            .await
            .map_err(|err| ClientError::Rpc(format!("task join error: {err}")))?
            .map_err(|err| ClientError::Transport(err.to_string()))?;

        let codex_path = attach
            .codex_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        let handle = attach
            .handle
            .map(|h| Arc::new(LocalServerProcessHandle::new(h)));

        Ok(LocalServerConnection {
            host: "127.0.0.1".to_string(),
            port: attach.port,
            websocket_url: format!("ws://127.0.0.1:{}", attach.port),
            attached_to_existing: attach.attached_to_existing,
            codex_path,
            handle,
        })
    }

    // ── Saved apps update ────────────────────────────────────────────────

    /// Choose a connected Codex host for a saved app, preferring its origin.
    pub fn saved_app_server_id(&self, directory: String, app_id: String) -> Option<String> {
        let app = crate::saved_apps::saved_app_get(directory, app_id)?.app;
        choose_saved_app_server_id(&self.inner, &app)
    }

    /// Generate replacement HTML on a remote host and persist it on the phone.
    /// Invalid responses leave the existing HTML and saved state untouched.
    pub async fn update_saved_app(
        &self,
        server_id: String,
        directory: String,
        app_id: String,
        user_prompt: String,
    ) -> SavedAppUpdateResult {
        let inner = Arc::clone(&self.inner);
        let rt = Arc::clone(&self.rt);
        let task = rt.spawn_blocking(move || {
            let inner = Arc::clone(&inner);
            let rt_for_block = Arc::clone(&crate::ffi::shared::shared_runtime());
            rt_for_block.block_on(async move {
                perform_update_saved_app(inner.as_ref(), server_id, directory, app_id, user_prompt)
                    .await
            })
        });
        match task.await {
            Ok(result) => result,
            Err(error) => SavedAppUpdateResult::Error {
                message: format!("update_saved_app task join failed: {error}"),
            },
        }
    }

    // ── Structured response (for app-mode `window.structuredResponse`) ───

    /// One-shot schema-constrained query against an ephemeral hidden
    /// thread. `cached_thread_id` is `None` on the first call from a saved
    /// app view; the helper starts an ephemeral thread, sends the turn with
    /// `output_schema` set, waits for the final assistant message, and
    /// returns the (JSON-string) response plus the resolved thread id so
    /// the host can cache it for subsequent calls in the same view.
    ///
    /// On a stale cached thread id (server reconnect, ephemeral thread
    /// gone), the helper transparently creates a fresh ephemeral thread
    /// and retries once. The caller should always overwrite its cache
    /// with the returned `thread_id`.
    pub async fn structured_response(
        &self,
        server_id: String,
        cached_thread_id: Option<String>,
        prompt: String,
        output_schema_json: String,
    ) -> StructuredResponseResult {
        let inner = Arc::clone(&self.inner);
        let rt = Arc::clone(&self.rt);
        let task = rt.spawn_blocking(move || {
            let inner = Arc::clone(&inner);
            let rt_for_block = Arc::clone(&crate::ffi::shared::shared_runtime());
            rt_for_block.block_on(async move {
                perform_structured_response(
                    inner.as_ref(),
                    server_id,
                    cached_thread_id,
                    prompt,
                    output_schema_json,
                )
                .await
            })
        });
        match task.await {
            Ok(result) => result,
            Err(error) => StructuredResponseResult::Error {
                message: format!("structured_response task join failed: {error}"),
            },
        }
    }

    // ── iPhone ↔ Mac proximity pairing ───────────────────────────────────

    /// Start the Mac-side pair host: bind a TCP listener, accept inbound
    /// pair WebSocket connections, and stage NI discovery tokens. Returns
    /// the bound port + Bonjour TXT entries so Swift can publish a
    /// `_litter-pair._tcp.` NetService alongside the Feature A local
    /// codex.
    ///
    /// `device_name` is the Mac's user-facing name (used as Bonjour
    /// instance name suggestion). `mac_id` is a stable per-Mac UUID
    /// (Swift persists this in UserDefaults — random UUID on first call).
    pub async fn start_pair_host(
        &self,
        device_name: String,
        mac_id: String,
        codex_port: u16,
    ) -> Result<PairHostStartResult, ClientError> {
        let rt = Arc::clone(&self.rt);
        let result = rt
            .spawn(
                async move { crate::pair::start_pair_host(device_name, mac_id, codex_port).await },
            )
            .await
            .map_err(|err| ClientError::Rpc(format!("task join error: {err}")))?
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let (handle, info) = result;
        Ok(PairHostStartResult { handle, info })
    }

    /// Open a WebSocket pair connection from the iPhone to the Mac's
    /// pair host. Sends the iPhone's hello inline; subsequent state is
    /// surfaced via the returned handle's `poll_event`.
    pub async fn pair_from_iphone(
        &self,
        host: String,
        port: u16,
        device_name: String,
        ni_discovery_token_b64: String,
    ) -> Result<Arc<crate::pair::PairClientHandle>, ClientError> {
        let rt = Arc::clone(&self.rt);
        rt.spawn(async move {
            crate::pair::pair_from_iphone(host, port, device_name, ni_discovery_token_b64).await
        })
        .await
        .map_err(|err| ClientError::Rpc(format!("task join error: {err}")))?
        .map_err(|err| ClientError::Transport(err.to_string()))
    }
}

/// Result of `AppClient::start_pair_host` — bundles the host handle (used
/// by Swift to drive the state machine) with the Bonjour publish info
/// (used by Swift to advertise a NetService).
#[derive(uniffi::Record)]
pub struct PairHostStartResult {
    pub handle: Arc<crate::pair::PairHostHandle>,
    pub info: crate::pair::PairServiceInfo,
}

/// Owning wrapper around a spawned `codex app-server` process. Exposed
/// to Swift/Kotlin as a UniFFI Object so the platform can hold it for the
/// lifetime of the app and call `stop()` on termination.
#[derive(uniffi::Object)]
pub struct LocalServerProcessHandle {
    inner: tokio::sync::Mutex<Option<crate::local_server::LocalServerHandle>>,
}

impl LocalServerProcessHandle {
    fn new(handle: crate::local_server::LocalServerHandle) -> Self {
        Self {
            inner: tokio::sync::Mutex::new(Some(handle)),
        }
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl LocalServerProcessHandle {
    /// Gracefully stop the spawned codex process. No-op if already stopped
    /// or if the handle represents an attached (not spawned) connection.
    pub async fn stop(&self) {
        let maybe = {
            let mut guard = self.inner.lock().await;
            guard.take()
        };
        if let Some(handle) = maybe {
            handle.stop().await;
        }
    }
}

/// Result of `AppClient::attach_or_spawn_local_server`.
#[derive(uniffi::Record)]
pub struct LocalServerConnection {
    /// Always `127.0.0.1` for a local codex. Included for symmetry with
    /// `connect_remote_server` on the Swift side.
    pub host: String,
    pub port: u16,
    pub websocket_url: String,
    /// `true` when we found an existing listener and did not spawn; in
    /// that case `handle` is `None`.
    pub attached_to_existing: bool,
    /// Resolved path to the `codex` binary when we spawned; `None` when
    /// we attached to an existing listener.
    pub codex_path: Option<String>,
    /// Lifetime handle for the spawned child. `None` when we attached.
    /// The caller must hold this (e.g. in the AppDelegate) and invoke
    /// `stop()` during `applicationWillTerminate`.
    pub handle: Option<Arc<LocalServerProcessHandle>>,
}

/// Execute a simple one-shot command on a remote server.
pub(crate) async fn exec_command_simple(
    client: &MobileClient,
    server_id: &str,
    command: &[&str],
    cwd: Option<&str>,
) -> Result<upstream::CommandExecResponse, ClientError> {
    let params = upstream::CommandExecParams {
        command: command.iter().map(|s| s.to_string()).collect(),
        process_id: None,
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        disable_output_cap: false,
        disable_timeout: false,
        timeout_ms: None,
        cwd: cwd
            .and_then(crate::remote_path::normalize_thread_cwd)
            .map(std::path::PathBuf::from),
        env: None,
        size: None,
        sandbox_policy: Some(upstream::SandboxPolicy::DangerFullAccess),
        permission_profile: None,
    };
    rpc(
        client,
        server_id,
        req!(server_id, OneOffCommandExec, params),
    )
    .await
}

/// Tolerant wire-compat mirror of `upstream::FuzzyFileSearchResponse`.
///
/// `match_type` was added upstream in March 2026 (commit 10eb3ec7f, "Simple
/// directory mentions"). Older `codex` server binaries omit it, which would
/// cause strict deserialization against `upstream::FuzzyFileSearchResponse`
/// to fail for the entire response. Default to `File` when absent.
#[derive(serde::Deserialize)]
struct WireFuzzyFileSearchResponse {
    #[serde(default)]
    files: Vec<WireFuzzyFileSearchResult>,
}

#[derive(serde::Deserialize)]
struct WireFuzzyFileSearchResult {
    root: String,
    path: String,
    #[serde(default = "default_match_type")]
    match_type: upstream::FuzzyFileSearchMatchType,
    file_name: String,
    score: u32,
    #[serde(default)]
    indices: Option<Vec<u32>>,
}

fn default_match_type() -> upstream::FuzzyFileSearchMatchType {
    upstream::FuzzyFileSearchMatchType::File
}

impl From<WireFuzzyFileSearchResult> for types::FileSearchResult {
    fn from(value: WireFuzzyFileSearchResult) -> Self {
        Self {
            root: value.root,
            path: value.path,
            match_type: value.match_type.into(),
            file_name: value.file_name,
            score: value.score,
            indices: value.indices,
        }
    }
}

async fn resolve_image_view_bytes(
    client: &MobileClient,
    server_id: &str,
    raw_path: &str,
    cwd: Option<&str>,
) -> Result<types::ResolvedImageViewResult, ClientError> {
    let source = ImageViewSource::parse(raw_path)
        .ok_or_else(|| ClientError::InvalidParams("image_view path is empty".to_string()))?;

    match source {
        ImageViewSource::InlineData(bytes) => Ok(types::ResolvedImageViewResult {
            path: raw_path.to_string(),
            bytes,
        }),
        ImageViewSource::FilePath(path) => {
            let local_path = if std::path::Path::new(&path).is_relative() {
                cwd.map(|base| std::path::Path::new(base).join(&path))
                    .unwrap_or_else(|| std::path::PathBuf::from(&path))
            } else {
                std::path::PathBuf::from(&path)
            };
            if let Ok(bytes) = std::fs::read(&local_path) {
                return Ok(types::ResolvedImageViewResult {
                    path: local_path.to_string_lossy().into_owned(),
                    bytes,
                });
            }

            if server_id.trim().is_empty() {
                return Err(ClientError::Rpc(
                    "Image path could not be read locally and no server is available.".to_string(),
                ));
            }

            let response = exec_command_simple_owned(
                client,
                server_id,
                image_read_command(&path),
                cwd.map(str::to_owned),
            )
            .await?;

            if response.exit_code != 0 {
                let stderr = response.stderr.trim();
                return Err(ClientError::Rpc(if stderr.is_empty() {
                    "Image read failed.".to_string()
                } else {
                    stderr.to_string()
                }));
            }

            let payload: String = response
                .stdout
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|error| {
                    ClientError::Serialization(format!("invalid image base64: {error}"))
                })?;

            Ok(types::ResolvedImageViewResult { path, bytes })
        }
    }
}

async fn exec_command_simple_owned(
    client: &MobileClient,
    server_id: &str,
    command: Vec<String>,
    cwd: Option<String>,
) -> Result<upstream::CommandExecResponse, ClientError> {
    let params = upstream::CommandExecParams {
        command,
        process_id: None,
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: Some(20_000_000),
        disable_output_cap: false,
        disable_timeout: false,
        timeout_ms: Some(15_000),
        cwd: cwd
            .as_deref()
            .and_then(crate::remote_path::normalize_thread_cwd)
            .map(std::path::PathBuf::from),
        env: None,
        size: None,
        sandbox_policy: None,
        permission_profile: None,
    };
    rpc(
        client,
        server_id,
        req!(server_id, OneOffCommandExec, params),
    )
    .await
}

fn image_read_command(path: &str) -> Vec<String> {
    if is_windows_path(path) {
        return vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $p = $args[0]; if ($p.StartsWith('~/') -or $p.StartsWith('~\\\\')) { $p = Join-Path $HOME $p.Substring(2) }; [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($p))".to_string(),
            path.to_string(),
        ];
    }

    vec![
        "/usr/bin/env".to_string(),
        "sh".to_string(),
        "-lc".to_string(),
        r#"path="$1"; case "$path" in "~/"*) path="$HOME/${path#~/}" ;; esac; base64 < "$path""#
            .to_string(),
        "sh".to_string(),
        path.to_string(),
    ]
}

fn is_windows_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[1] == b':'
        && bytes[0].is_ascii_alphabetic()
        && (bytes[2] == b'\\' || bytes[2] == b'/'))
        || path.starts_with("\\\\")
}

struct RemotePetScanEntry {
    summary: types::AppPetSummary,
    manifest_json: String,
}

async fn scan_remote_pets(
    client: &MobileClient,
    server_id: &str,
) -> Result<Vec<RemotePetScanEntry>, ClientError> {
    ensure_pet_runtime_available(client, server_id)?;

    let script = r#"root="${CODEX_HOME:-$HOME/.codex}/pets"
[ -d "$root" ] || exit 0
for manifest in "$root"/*/pet.json; do
  [ -f "$manifest" ] || continue
  dir=${manifest%/pet.json}
  printf '%s\t' "$(printf '%s' "$dir" | base64 | tr -d '\n')"
  base64 < "$manifest" | tr -d '\n'
  printf '\n'
done"#;
    let response = exec_command_simple_owned(
        client,
        server_id,
        vec![
            "/usr/bin/env".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            script.to_string(),
        ],
        None,
    )
    .await?;
    if response.exit_code != 0 {
        let stderr = response.stderr.trim();
        return Err(ClientError::Rpc(if stderr.is_empty() {
            "pet scan failed".to_string()
        } else {
            stderr.to_string()
        }));
    }

    let mut entries = Vec::new();
    for line in response.stdout.lines() {
        let Some((path_b64, manifest_b64)) = line.split_once('\t') else {
            continue;
        };
        let path = decode_base64_utf8(path_b64, "pet path")?;
        let manifest_json = decode_base64_utf8(manifest_b64, "pet manifest")?;
        let mut summary = crate::pets::summary_from_manifest(path.clone(), &manifest_json, false);
        if let Some(spritesheet_file) = summary.spritesheet_path.as_deref() {
            let spritesheet_path = crate::pets::local_spritesheet_path(&path, spritesheet_file)
                .map_err(ClientError::Serialization)?;
            summary.has_valid_spritesheet =
                remote_file_exists(client, server_id, &spritesheet_path).await?;
            if summary.has_valid_spritesheet {
                summary.validation_error = None;
            } else if summary.validation_error.is_none() {
                summary.validation_error = Some(format!("{spritesheet_file} is missing"));
            }
        }
        entries.push(RemotePetScanEntry {
            summary,
            manifest_json,
        });
    }
    entries.sort_by(|a, b| {
        a.summary
            .display_name
            .to_lowercase()
            .cmp(&b.summary.display_name.to_lowercase())
    });
    Ok(entries)
}

fn ensure_pet_runtime_available(client: &MobileClient, server_id: &str) -> Result<(), ClientError> {
    let runtime_kinds = client
        .get_session(server_id)
        .map_err(|error| ClientError::Rpc(error.to_string()))?
        .runtime_kinds();
    if runtime_kinds.contains(&"codex".to_string()) {
        return Ok(());
    }
    Err(ClientError::Rpc(
        "pets require a connected Codex runtime; select the Codex Alleycat agent or connect to a Codex server".to_string(),
    ))
}

async fn remote_file_exists(
    client: &MobileClient,
    server_id: &str,
    path: &str,
) -> Result<bool, ClientError> {
    let response =
        exec_command_simple_owned(client, server_id, file_exists_command(path), None).await?;
    Ok(response.exit_code == 0)
}

async fn read_remote_file_bytes(
    client: &MobileClient,
    server_id: &str,
    path: &str,
) -> Result<Vec<u8>, ClientError> {
    let response =
        exec_command_simple_owned(client, server_id, image_read_command(path), None).await?;
    if response.exit_code != 0 {
        let stderr = response.stderr.trim();
        return Err(ClientError::Rpc(if stderr.is_empty() {
            "file read failed".to_string()
        } else {
            stderr.to_string()
        }));
    }
    let payload: String = response
        .stdout
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|error| ClientError::Serialization(format!("invalid file base64: {error}")))
}

fn decode_base64_utf8(value: &str, label: &str) -> Result<String, ClientError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|error| ClientError::Serialization(format!("invalid {label} base64: {error}")))?;
    String::from_utf8(bytes)
        .map_err(|error| ClientError::Serialization(format!("invalid {label} utf8: {error}")))
}

fn file_exists_command(path: &str) -> Vec<String> {
    if is_windows_path(path) {
        return vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "$p = $args[0]; if ($p.StartsWith('~/') -or $p.StartsWith('~\\\\')) { $p = Join-Path $HOME $p.Substring(2) }; if (Test-Path -LiteralPath $p -PathType Leaf) { exit 0 } else { exit 1 }".to_string(),
            path.to_string(),
        ];
    }
    vec![
        "/usr/bin/env".to_string(),
        "sh".to_string(),
        "-lc".to_string(),
        r#"path="$1"; case "$path" in "~/"*) path="$HOME/${path#~/}" ;; esac; test -f "$path""#
            .to_string(),
        "sh".to_string(),
        path.to_string(),
    ]
}

enum ImageViewSource {
    InlineData(Vec<u8>),
    FilePath(String),
}

impl ImageViewSource {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(bytes) = inline_image_data(trimmed) {
            return Some(Self::InlineData(bytes));
        }

        if let Some(path) = normalized_image_path(trimmed) {
            return Some(Self::FilePath(path));
        }

        None
    }
}

fn normalized_image_path(raw: &str) -> Option<String> {
    if raw.starts_with("file://") {
        let url = Url::parse(raw).ok()?;
        if url.scheme() == "file" {
            return url
                .to_file_path()
                .ok()
                .map(|path| path.to_string_lossy().into_owned());
        }
    }

    if raw.starts_with('/')
        || raw.starts_with("~/")
        || raw.starts_with("\\\\")
        || is_windows_path(raw)
    {
        return Some(raw.to_string());
    }

    if !raw.contains("://") && !raw.starts_with("data:") {
        return Some(raw.to_string());
    }

    None
}

fn inline_image_data(raw: &str) -> Option<Vec<u8>> {
    let source = raw.strip_prefix("data:image/")?;
    let (_, payload) = source.split_once(";base64,")?;
    let normalized: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(normalized)
        .ok()
}

/// Return the "Apps saved in this thread so far: …" context line for
/// the given `thread_id`, or `None` when:
/// - `thread_id` is `None` (no thread yet, nothing to reference),
/// - the saved-apps directory hasn't been registered by the platform,
/// - the directory has no apps for this thread.
fn saved_apps_context_line(
    client: &MobileClient,
    server_id: &str,
    thread_id: Option<&str>,
) -> Option<String> {
    let thread_id = thread_id?;
    let directory = {
        let guard = client
            .saved_apps_directory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.clone()?
    };
    let apps = crate::saved_apps::saved_apps_for_thread(directory, thread_id.to_string())
        .into_iter()
        .filter(|app| app.origin_server_id.as_deref() == Some(server_id))
        .collect::<Vec<_>>();
    if apps.is_empty() {
        return None;
    }
    let joined = apps
        .iter()
        .map(|app| format!("{} ({})", app.app_id, app.title))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("Apps saved in this thread so far: {joined}"))
}

/// Prepend the saved-apps context line to `existing` if the thread has
/// any saved apps. If there are no apps (or no thread yet), returns
/// `existing` unchanged so callers can feed this directly into the RPC
/// request params.
fn splice_saved_apps_context(
    client: &MobileClient,
    server_id: &str,
    thread_id: Option<&str>,
    existing: Option<String>,
) -> Option<String> {
    let Some(line) = saved_apps_context_line(client, server_id, thread_id) else {
        return existing;
    };
    Some(match existing {
        Some(prev) if !prev.trim().is_empty() => format!("{line}\n\n{prev}"),
        _ => line,
    })
}

/// Prepend the generative-UI preamble when this thread is registering the
/// `show_widget` / `visualize_read_me` dynamic tools. Rust registers the
/// tools for supported interactive runtimes before injecting this nudge.
fn splice_generative_ui_preamble(
    dynamic_tools: &Option<Vec<crate::types::models::AppDynamicToolSpec>>,
    existing: Option<String>,
) -> Option<String> {
    let has_show_widget = dynamic_tools
        .as_ref()
        .map(|tools| tools.iter().any(|t| t.name == "show_widget"))
        .unwrap_or(false);
    if !has_show_widget {
        return existing;
    }
    let preamble = crate::widget_guidelines::GENERATIVE_UI_PREAMBLE;
    Some(match existing {
        Some(prev) if !prev.trim().is_empty() => format!("{preamble}\n\n{prev}"),
        _ => preamble.to_string(),
    })
}

// ── Saved apps update helpers ────────────────────────────────────────────

/// Typed result of `AppClient::update_saved_app`.
#[derive(uniffi::Enum)]
pub enum SavedAppUpdateResult {
    Success { app: crate::saved_apps::SavedApp },
    Error { message: String },
}

/// Typed result of `AppClient::structured_response`.
#[derive(uniffi::Enum)]
pub enum StructuredResponseResult {
    Success {
        /// The ephemeral thread id the call landed on. The caller MUST
        /// overwrite its cache from this value on every success — on
        /// stale-thread recovery this differs from the `cached_thread_id`
        /// passed in.
        thread_id: String,
        /// Raw JSON string matching the caller's `output_schema`. The
        /// caller is expected to `JSON.parse` it.
        response_json: String,
    },
    Error {
        message: String,
    },
}

const STRUCTURED_RESPONSE_TIMEOUT_SECS: u64 = 60;

const SAVED_APP_UPDATE_TIMEOUT_SECS: u64 = 120;

fn is_stale_thread_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("thread not found")
        || lower.contains("conversation not found")
        || lower.contains("unknown thread")
        || lower.contains("no such thread")
}

async fn start_ephemeral_thread_for_structured(
    client: &crate::MobileClient,
    server_id: &str,
) -> Result<String, String> {
    let start_params = upstream::ThreadStartParams {
        model: None,
        model_provider: None,
        service_tier: None,
        cwd: None,
        runtime_workspace_roots: None,
        approval_policy: None,
        approvals_reviewer: None,
        sandbox: None,
        permissions: None,
        config: None,
        service_name: None,
        base_instructions: None,
        developer_instructions: None,
        personality: None,
        ephemeral: Some(true),
        session_start_source: None,
        thread_source: None,
        environments: None,
        dynamic_tools: None,
        mock_experimental_field: None,
        experimental_raw_events: false,
        ..Default::default()
    };
    let response: upstream::ThreadStartResponse = client
        .request_typed_for_server_runtime(
            server_id,
            "codex".into(),
            upstream::ClientRequest::ThreadStart {
                request_id: upstream::RequestId::Integer(next_request_id()),
                params: start_params,
            },
        )
        .await
        .map_err(|e| format!("thread/start failed: {e}"))?;
    let key = types::ThreadKey {
        server_id: server_id.into(),
        thread_id: response.thread.id.clone(),
    };
    client.note_thread_runtime(key, "codex".into());
    Ok(response.thread.id)
}

async fn run_structured_turn(
    client: &crate::MobileClient,
    server_id: &str,
    thread_id: &str,
    prompt: &str,
    output_schema: serde_json::Value,
    effort: Option<crate::types::ReasoningEffort>,
) -> Result<String, StructuredTurnError> {
    // Subscribe BEFORE sending the turn so we don't miss a very fast
    // completion. `UiEvent` is the typed, thread-scoped event stream the
    // mobile client already fans out to the store.
    let mut events_rx = client.event_processor.subscribe();

    let turn_params = upstream::TurnStartParams {
        thread_id: thread_id.to_string(),
        input: vec![upstream::UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }],
        responsesapi_client_metadata: None,
        cwd: None,
        runtime_workspace_roots: None,
        approval_policy: None,
        approvals_reviewer: None,
        sandbox_policy: None,
        environments: None,
        permissions: None,
        model: None,
        service_tier: None,
        effort: effort.map(crate::types::server_requests::reasoning_effort_into_upstream),
        summary: None,
        personality: None,
        output_schema: Some(output_schema),
        collaboration_mode: None,
        ..Default::default()
    };
    let turn_outcome: Result<upstream::TurnStartResponse, _> = client
        .request_typed_for_server(
            server_id,
            upstream::ClientRequest::TurnStart {
                request_id: upstream::RequestId::Integer(next_request_id()),
                params: turn_params,
            },
        )
        .await;
    let turn_id = match turn_outcome {
        Ok(response) => response.turn.id,
        Err(e) if is_stale_thread_error(&e) => return Err(StructuredTurnError::StaleThread),
        Err(e) => {
            return Err(StructuredTurnError::Fatal(format!(
                "turn/start failed: {e}"
            )));
        }
    };

    let wait_outcome = tokio::time::timeout(
        std::time::Duration::from_secs(STRUCTURED_RESPONSE_TIMEOUT_SECS),
        async {
            let mut last_agent_text: Option<String> = None;
            loop {
                match events_rx.recv().await {
                    Ok(crate::session::events::UiEvent::ItemCompleted { key, notification })
                        if key.server_id == server_id
                            && key.thread_id == thread_id
                            && notification.turn_id == turn_id =>
                    {
                        if let upstream::ThreadItem::AgentMessage { text, .. } = &notification.item
                        {
                            last_agent_text = Some(text.clone());
                        }
                    }
                    Ok(crate::session::events::UiEvent::TurnCompleted {
                        key,
                        turn_id: completed_id,
                        error,
                    }) if key.thread_id == thread_id
                        && key.server_id == server_id
                        && completed_id == turn_id =>
                    {
                        return Ok((last_agent_text, error));
                    }
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err("event stream closed".to_string());
                    }
                }
            }
        },
    )
    .await;

    match wait_outcome {
        Ok(Ok((_, Some(err)))) => Err(StructuredTurnError::Fatal(err)),
        Ok(Ok((Some(text), None))) => Ok(text),
        Ok(Ok((None, None))) => Err(StructuredTurnError::Fatal(
            "turn completed with no assistant message".to_string(),
        )),
        Ok(Err(msg)) => Err(StructuredTurnError::Fatal(msg)),
        Err(_) => Err(StructuredTurnError::Fatal(format!(
            "timed out after {STRUCTURED_RESPONSE_TIMEOUT_SECS}s waiting for structured response"
        ))),
    }
}

enum StructuredTurnError {
    StaleThread,
    Fatal(String),
}

async fn perform_structured_response(
    client: &crate::MobileClient,
    server_id: String,
    cached_thread_id: Option<String>,
    prompt: String,
    output_schema_json: String,
) -> StructuredResponseResult {
    if prompt.trim().is_empty() {
        return StructuredResponseResult::Error {
            message: "prompt is empty".to_string(),
        };
    }
    let schema: serde_json::Value = match serde_json::from_str(&output_schema_json) {
        Ok(v) => v,
        Err(e) => {
            return StructuredResponseResult::Error {
                message: format!("invalid responseFormat JSON schema: {e}"),
            };
        }
    };

    // First attempt: use cached id if provided, otherwise start fresh.
    let mut thread_id = match cached_thread_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => match start_ephemeral_thread_for_structured(client, &server_id).await {
            Ok(id) => id,
            Err(e) => return StructuredResponseResult::Error { message: e },
        },
    };

    match run_structured_turn(
        client,
        &server_id,
        &thread_id,
        &prompt,
        schema.clone(),
        None,
    )
    .await
    {
        Ok(text) => StructuredResponseResult::Success {
            thread_id,
            response_json: text,
        },
        Err(StructuredTurnError::StaleThread) => {
            // Cached thread is gone. Reseat and retry exactly once.
            thread_id = match start_ephemeral_thread_for_structured(client, &server_id).await {
                Ok(id) => id,
                Err(e) => {
                    return StructuredResponseResult::Error {
                        message: format!("stale-thread recovery failed: {e}"),
                    };
                }
            };
            match run_structured_turn(client, &server_id, &thread_id, &prompt, schema, None).await {
                Ok(text) => StructuredResponseResult::Success {
                    thread_id,
                    response_json: text,
                },
                Err(StructuredTurnError::StaleThread) => StructuredResponseResult::Error {
                    message: "thread became stale again on retry".to_string(),
                },
                Err(StructuredTurnError::Fatal(msg)) => {
                    StructuredResponseResult::Error { message: msg }
                }
            }
        }
        Err(StructuredTurnError::Fatal(msg)) => StructuredResponseResult::Error { message: msg },
    }
}

/// Resolve a connected Codex host in one place for both native apps.
fn choose_saved_app_server_id(
    client: &MobileClient,
    app: &crate::saved_apps::SavedApp,
) -> Option<String> {
    let snapshot = client.app_store.snapshot();
    let mut connected = snapshot
        .servers
        .values()
        .filter(|server| {
            !server.is_local
                && matches!(server.health, crate::store::ServerHealthSnapshot::Connected)
        })
        .filter(|server| {
            client
                .get_session(&server.server_id)
                .is_ok_and(|s| s.runtime_kinds().iter().any(|r| r == "codex"))
        })
        .map(|server| server.server_id.clone())
        .collect::<Vec<_>>();
    connected.sort();
    let origin = app.origin_server_id.clone().or_else(|| {
        let id = app.origin_thread_id.as_deref()?;
        let mut matches = snapshot.threads.keys().filter(|key| key.thread_id == id);
        let first = matches.next()?;
        matches.next().is_none().then(|| first.server_id.clone())
    });
    origin
        .filter(|id| connected.contains(id))
        .or_else(|| {
            snapshot
                .active_thread
                .as_ref()
                .map(|key| key.server_id.clone())
                .filter(|id| connected.contains(id))
        })
        .or_else(|| connected.into_iter().next())
}

async fn perform_update_saved_app(
    client: &crate::MobileClient,
    server_id: String,
    directory: String,
    app_id: String,
    user_prompt: String,
) -> SavedAppUpdateResult {
    match update_saved_app_remotely(client, &server_id, &directory, &app_id, &user_prompt).await {
        Ok(app) => SavedAppUpdateResult::Success { app },
        Err(message) => SavedAppUpdateResult::Error { message },
    }
}

async fn update_saved_app_remotely(
    client: &MobileClient,
    server_id: &str,
    directory: &str,
    app_id: &str,
    user_prompt: &str,
) -> Result<crate::saved_apps::SavedApp, String> {
    if user_prompt.trim().is_empty() {
        return Err("Update prompt is empty".into());
    }
    let current = crate::saved_apps::saved_app_get(directory.into(), app_id.into())
        .ok_or_else(|| format!("saved app '{app_id}' not found"))?;
    let session = client.get_session(server_id).map_err(|e| e.to_string())?;
    if !session
        .runtime_kinds()
        .iter()
        .any(|runtime| runtime == "codex")
    {
        return Err("Saved app updates require a connected Codex host".into());
    }
    // Never send device paths to the host. Only HTML and a state-shape summary
    // cross RPC; the state file remains owned by the phone.
    let shape = crate::saved_apps::abbreviated_state_shape(directory, app_id).unwrap_or_default();
    let instructions =
        build_saved_app_update_seed(&current.app.title, current.app.schema_version, &shape);
    let same_origin = current
        .app
        .origin_server_id
        .as_deref()
        .is_none_or(|id| id == server_id);
    let (model, effort) = if same_origin {
        inherited_settings_for_origin(client, server_id, current.app.origin_thread_id.as_deref())
    } else {
        (None, None)
    };
    let response: upstream::ThreadStartResponse = tokio::time::timeout(
        Duration::from_secs(30),
        rpc_runtime(
            client,
            server_id,
            "codex".into(),
            upstream::ClientRequest::ThreadStart {
                request_id: upstream::RequestId::Integer(next_request_id()),
                params: upstream::ThreadStartParams {
                    model,
                    developer_instructions: Some(instructions),
                    ephemeral: Some(true),
                    sandbox: Some(upstream::SandboxMode::ReadOnly),
                    approval_policy: Some(upstream::AskForApproval::OnRequest),
                    ..Default::default()
                },
            },
        ),
    )
    .await
    .map_err(|_| "Saved app thread/start timed out".to_string())?
    .map_err(|e| e.to_string())?;
    let thread_id = response.thread.id;
    client.note_thread_runtime(
        crate::types::ThreadKey {
            server_id: server_id.into(),
            thread_id: thread_id.clone(),
        },
        "codex".into(),
    );
    let prompt =
        serde_json::json!({"requested_change": user_prompt, "current_html": current.widget_html})
            .to_string();
    let schema = serde_json::json!({
        "type": "object", "properties": {"widget_html": {"type": "string"}},
        "required": ["widget_html"], "additionalProperties": false
    });
    let outcome = tokio::time::timeout(
        Duration::from_secs(SAVED_APP_UPDATE_TIMEOUT_SECS),
        run_structured_turn(client, server_id, &thread_id, &prompt, schema, effort),
    )
    .await;
    // Best-effort archive on success and failure; disconnected hosts may not
    // accept cleanup. Keep it bounded so it cannot hold the update UI open.
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        rpc_runtime::<upstream::ThreadArchiveResponse>(
            client,
            server_id,
            "codex".into(),
            upstream::ClientRequest::ThreadArchive {
                request_id: upstream::RequestId::Integer(next_request_id()),
                params: upstream::ThreadArchiveParams { thread_id },
            },
        ),
    )
    .await;
    let json = outcome
        .map_err(|_| "Saved app update timed out".to_string())?
        .map_err(|error| match error {
            StructuredTurnError::StaleThread => {
                "Saved app update thread is no longer available".into()
            }
            StructuredTurnError::Fatal(message) => message,
        })?;
    #[derive(serde::Deserialize)]
    struct UpdatedWidget {
        widget_html: String,
    }
    let updated: UpdatedWidget =
        serde_json::from_str(&json).map_err(|e| format!("Invalid saved app response: {e}"))?;
    if updated.widget_html.trim().is_empty() {
        return Err("The host returned empty app HTML".into());
    }
    crate::saved_apps::saved_app_replace_html(
        directory.into(),
        app_id.into(),
        updated.widget_html,
        current.app.width,
        current.app.height,
    )
    .map_err(|e| e.to_string())
}

type InheritedSettings = (
    Option<String>,
    Option<crate::types::models::ReasoningEffort>,
);

/// Look up an origin thread in the app store and extract its effective
/// model / reasoning settings so the saved-app update thread can run with
/// the same model configuration the user chose for the source conversation.
/// Returns `(None, None)` when the thread is unknown, never hydrated, or
/// belongs to a different server.
fn inherited_settings_for_origin(
    client: &crate::MobileClient,
    server_id: &str,
    origin_thread_id: Option<&str>,
) -> InheritedSettings {
    let Some(thread_id) = origin_thread_id.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }) else {
        return (None, None);
    };

    let snapshot = client.app_store.snapshot();
    let key = crate::types::ThreadKey {
        server_id: server_id.to_string(),
        thread_id,
    };
    let Some(thread) = snapshot.threads.get(&key) else {
        return (None, None);
    };

    if thread.agent_runtime_kind != "codex" {
        return (None, None);
    }
    let model = thread
        .model
        .as_ref()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    let effort = crate::types::reasoning_effort_from_wire_value(thread.reasoning_effort.clone());
    (model, effort)
}

fn build_saved_app_update_seed(
    title: &str,
    schema_version: u32,
    state_shape_summary: &str,
) -> String {
    let guidelines = crate::widget_guidelines::get_guidelines(&["app".into()]);
    format!(
        "Update the saved app {title:?} using the current_html and requested_change in the user message. \
Return the complete replacement HTML in the widget_html JSON field. Do not create or edit files. \
The app lives on the phone, not on this host. Preserve window.loadAppState()/window.saveAppState() \
and existing user data. Keep the current state schema version {schema_version}; handle old state \
defensively on load. Keep HTML/CSS/JS self-contained. State shape: {state_shape_summary}\n\n{guidelines}"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ImageViewSource, THREAD_LIST_HYDRATION_BUDGET, image_read_command, is_mobile_hidden_skill,
        list_runtime_kinds, normalize_model_info_for_runtime, normalized_image_path,
        runtime_exposes_model_choices, splice_generative_ui_preamble, thread_list_can_prune,
        thread_list_hydration_budget,
    };
    use crate::types::models::{AbsolutePath, AppDynamicToolSpec, SkillMetadata, SkillScope};
    use crate::types::{AgentRuntimeKind, ModelInfo, ReasoningEffort};
    use crate::widget_guidelines::GENERATIVE_UI_PREAMBLE;

    fn show_widget_spec() -> AppDynamicToolSpec {
        AppDynamicToolSpec {
            name: "show_widget".to_string(),
            description: "test".to_string(),
            input_schema_json: "{}".to_string(),
            defer_loading: false,
        }
    }

    fn skill_metadata(name: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: String::new(),
            short_description: None,
            interface: None,
            dependencies: None,
            path: AbsolutePath {
                value: path.to_string(),
            },
            scope: SkillScope::System,
            enabled: true,
        }
    }

    fn test_model(id: &str, runtime_kind: AgentRuntimeKind) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            model: id.to_string(),
            upgrade: None,
            upgrade_model: None,
            upgrade_copy: None,
            model_link: None,
            migration_markdown: None,
            availability_nux_message: None,
            display_name: id.to_string(),
            description: String::new(),
            hidden: false,
            supported_reasoning_efforts: Vec::new(),
            default_reasoning_effort: ReasoningEffort::Medium,
            input_modalities: Vec::new(),
            supports_personality: false,
            is_default: false,
            agent_runtime_kind: runtime_kind,
            provider_id: None,
            entry_kind: crate::types::ModelEntryKind::Model,
            picker_name: String::new(),
            provider_label: None,
        }
    }

    #[test]
    fn model_picker_queries_every_chat_runtime() {
        assert!(runtime_exposes_model_choices("codex"));
        assert!(runtime_exposes_model_choices("pi"));
        assert!(runtime_exposes_model_choices("local-studio"));
        assert!(!runtime_exposes_model_choices("shell"));
        let mut pi = test_model("openrouter/ai21/jamba", "pi".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut pi,
            "pi".to_string(),
            None
        ));
        assert_eq!(pi.provider_id.as_deref(), Some("ai21"));
        assert!(pi.supported_reasoning_efforts.is_empty());
        let mut codex = test_model("openai/gpt-6-codex", "codex".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut codex,
            "codex".to_string(),
            None
        ));
        assert_eq!(codex.provider_id, None);

        let mut local_studio = test_model("controller/qwen3-coder", "local-studio".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut local_studio,
            "local-studio".to_string(),
            None
        ));
        assert!(local_studio.supported_reasoning_efforts.is_empty());
    }

    #[test]
    fn model_picker_classifies_modes_and_labels_providers() {
        let amp_modes = vec![
            "low".to_string(),
            "medium".to_string(),
            "high".to_string(),
            "ultra".to_string(),
        ];
        let mut builtin = test_model("amp/High", "amp".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut builtin,
            "amp".to_string(),
            Some(&amp_modes)
        ));
        assert_eq!(builtin.entry_kind, crate::types::ModelEntryKind::Mode);
        assert_eq!(builtin.picker_name, "high");

        let mut plugin = test_model("glm-5.2", "amp".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut plugin,
            "amp".to_string(),
            Some(&amp_modes)
        ));
        assert_eq!(plugin.entry_kind, crate::types::ModelEntryKind::PluginMode);
        assert_eq!(plugin.picker_name, "glm-5.2");

        let mut pi = test_model("openrouter/x-ai/grok-5", "pi".to_string());
        pi.display_name = "x-ai/grok-5 (openrouter)".to_string();
        assert!(normalize_model_info_for_runtime(
            &mut pi,
            "pi".to_string(),
            None
        ));
        assert_eq!(pi.entry_kind, crate::types::ModelEntryKind::Model);
        assert_eq!(pi.provider_id.as_deref(), Some("x-ai"));
        assert_eq!(pi.provider_label.as_deref(), Some("xAI"));
        assert_eq!(pi.picker_name, "grok-5");

        let mut custom = test_model("my-lab/qwen", "pi".to_string());
        assert!(normalize_model_info_for_runtime(
            &mut custom,
            "pi".to_string(),
            None
        ));
        assert_eq!(custom.provider_label.as_deref(), Some("My Lab"));
        assert_eq!(custom.picker_name, "qwen");
    }

    #[test]
    fn thread_list_only_queries_runtimes_the_controller_exposes() {
        let local_studio = vec!["local-studio".to_string()];
        assert_eq!(list_runtime_kinds(None, &local_studio), local_studio);
        assert!(list_runtime_kinds(Some(vec!["codex".to_string()]), &local_studio).is_empty());
    }

    #[test]
    fn claude_catalog_preserves_host_models_and_default() {
        let mut custom = test_model("my-bedrock-deployment", "codex".to_string());
        custom.provider_id = Some("bedrock".to_string());
        custom.is_default = true;
        assert!(normalize_model_info_for_runtime(
            &mut custom,
            "claude".to_string(),
            None
        ));
        assert_eq!(custom.agent_runtime_kind, "claude");
        assert_eq!(custom.provider_id.as_deref(), Some("bedrock"));
        let models = vec![custom];
        assert_eq!(models.len(), 1);
        assert_eq!(models.iter().filter(|model| model.is_default).count(), 1);
        assert_eq!(
            models.iter().find(|model| model.is_default).unwrap().id,
            "my-bedrock-deployment"
        );
    }

    #[test]
    fn thread_list_hydration_budget_bounds_recents_and_skips_scoped_queries() {
        let request = |limit: Option<u32>,
                       cursor: Option<&str>,
                       search_term: Option<&str>,
                       use_state_db_only: bool| {
            crate::types::AppListThreadsRequest {
                cursor: cursor.map(str::to_string),
                limit,
                sort_key: None,
                sort_direction: None,
                model_providers: None,
                source_kinds: None,
                archived: None,
                cwd: None,
                search_term: search_term.map(str::to_string),
                use_state_db_only,
                runtime_kinds: None,
            }
        };

        let mut scoped = request(Some(100), None, None, false);
        assert!(thread_list_can_prune(&scoped));
        scoped.runtime_kinds = Some(vec!["claude".to_string()]);
        assert!(!thread_list_can_prune(&scoped));
        scoped.runtime_kinds = None;
        scoped.cwd = Some("/one-project".to_string());
        assert!(!thread_list_can_prune(&scoped));
        scoped.cwd = None;
        scoped.archived = Some(true);
        assert!(!thread_list_can_prune(&scoped));

        assert_eq!(
            thread_list_hydration_budget(&request(None, None, None, false)),
            Some(THREAD_LIST_HYDRATION_BUDGET)
        );
        assert_eq!(
            thread_list_hydration_budget(&request(Some(100), None, None, false)),
            Some(100)
        );
        assert_eq!(
            thread_list_hydration_budget(&request(None, None, Some("   "), false)),
            Some(THREAD_LIST_HYDRATION_BUDGET)
        );
        assert_eq!(
            thread_list_hydration_budget(&request(None, Some("cursor"), None, false)),
            None
        );
        assert_eq!(
            thread_list_hydration_budget(&request(None, None, Some("query"), false)),
            None
        );
        assert_eq!(
            thread_list_hydration_budget(&request(None, None, None, true)),
            None
        );
    }

    #[test]
    fn claude_catalog_preserves_advertised_capabilities() {
        let mut advertised = test_model("sonnet", "claude".to_string());
        advertised.description = "Host capabilities".to_string();
        advertised.supported_reasoning_efforts.clear();
        let models = vec![advertised];
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].description, "Host capabilities");
        assert!(models[0].supported_reasoning_efforts.is_empty());
    }

    #[test]
    fn amp_plugin_modes_preserve_native_identity_and_default() {
        let mut model = test_model("my-plugin-mode", "amp".into());
        model.display_name = "Team plugin".into();
        model.is_default = true;
        let amp_modes = vec!["low".to_string(), "medium".to_string()];
        assert!(normalize_model_info_for_runtime(
            &mut model,
            "amp".into(),
            Some(&amp_modes)
        ));
        assert_eq!(model.id, "my-plugin-mode");
        assert_eq!(model.display_name, "Team plugin");
        assert!(model.is_default);
        assert_eq!(model.entry_kind, crate::types::ModelEntryKind::PluginMode);
        assert_eq!(model.picker_name, "Team plugin");
    }

    #[test]
    fn shell_runtime_does_not_expose_model_choices() {
        assert!(!runtime_exposes_model_choices("shell"));
        assert!(runtime_exposes_model_choices("amp"));
        assert!(runtime_exposes_model_choices("codex"));
    }

    #[test]
    fn preamble_prepended_when_show_widget_registered() {
        let tools = Some(vec![show_widget_spec()]);
        let result = splice_generative_ui_preamble(&tools, Some("user instructions".to_string()));
        let out = result.expect("expected Some");
        assert!(out.starts_with(GENERATIVE_UI_PREAMBLE));
        assert!(out.ends_with("user instructions"));
    }

    #[test]
    fn preamble_skipped_without_show_widget() {
        let other = AppDynamicToolSpec {
            name: "list_servers".to_string(),
            description: "x".to_string(),
            input_schema_json: "{}".to_string(),
            defer_loading: false,
        };
        let tools = Some(vec![other]);
        let result = splice_generative_ui_preamble(&tools, Some("user instructions".to_string()));
        assert_eq!(result.as_deref(), Some("user instructions"));
    }

    #[test]
    fn preamble_used_alone_when_no_existing_instructions() {
        let tools = Some(vec![show_widget_spec()]);
        assert_eq!(
            splice_generative_ui_preamble(&tools, None).as_deref(),
            Some(GENERATIVE_UI_PREAMBLE)
        );
    }

    #[test]
    fn preamble_skipped_when_no_dynamic_tools() {
        assert_eq!(
            splice_generative_ui_preamble(&None, Some("keep".to_string())).as_deref(),
            Some("keep")
        );
    }

    #[test]
    fn mobile_hides_imagegen_skill_by_name() {
        assert!(is_mobile_hidden_skill(&skill_metadata(
            "imagegen",
            "/Users/me/.codex/skills/.system/imagegen"
        )));
        assert!(is_mobile_hidden_skill(&skill_metadata(
            "ImageGen",
            "/Users/me/.codex/skills/user/imagegen"
        )));
    }

    #[test]
    fn mobile_hides_system_imagegen_skill_by_path() {
        assert!(is_mobile_hidden_skill(&skill_metadata(
            "AI Images",
            "/Users/me/.codex/skills/.system/imagegen/"
        )));
    }

    #[test]
    fn mobile_keeps_other_skills() {
        assert!(!is_mobile_hidden_skill(&skill_metadata(
            "browser",
            "/Users/me/.codex/skills/.system/browser"
        )));
    }

    #[test]
    fn parses_inline_image_data() {
        let source = ImageViewSource::parse("data:image/png;base64,SGVsbG8=");
        match source {
            Some(ImageViewSource::InlineData(bytes)) => assert_eq!(bytes, b"Hello"),
            _ => panic!("expected inline image data"),
        }
    }

    #[test]
    fn normalizes_file_url_path() {
        assert_eq!(
            normalized_image_path("file:///tmp/example.png").as_deref(),
            Some("/tmp/example.png")
        );
    }

    #[test]
    fn builds_posix_image_read_command_with_remote_tilde_expansion() {
        let command = image_read_command("~/image.png");
        assert_eq!(command[0], "/usr/bin/env");
        assert_eq!(command[1], "sh");
        assert!(command[3].contains(r#"${path#~/}"#));
    }

    mod plugin_list {
        use super::super::shape_plugin_list;
        use codex_app_server_protocol as upstream;
        use codex_utils_absolute_path::AbsolutePathBuf;

        fn iface(display_name: &str, short_description: &str) -> upstream::PluginInterface {
            upstream::PluginInterface {
                display_name: Some(display_name.into()),
                short_description: Some(short_description.into()),
                long_description: None,
                developer_name: None,
                category: None,
                capabilities: Vec::new(),
                website_url: None,
                privacy_policy_url: None,
                terms_of_service_url: None,
                default_prompt: None,
                brand_color: None,
                composer_icon: None,
                composer_icon_url: None,
                logo: None,
                logo_url: None,
                logo_dark: None,
                logo_url_dark: None,
                screenshots: Vec::new(),
                screenshot_urls: Vec::new(),
            }
        }

        fn summary(
            id: &str,
            name: &str,
            installed: bool,
            enabled: bool,
            install_policy: upstream::PluginInstallPolicy,
            display: Option<&str>,
        ) -> upstream::PluginSummary {
            upstream::PluginSummary {
                id: id.into(),
                remote_plugin_id: None,
                version: None,
                installed_at: None,
                disabled_reason: None,
                eligible_plan_types: None,
                install_policy_source: None,
                must_show_installation_interstitial: None,
                local_version: None,
                name: name.into(),
                share_context: None,
                source: upstream::PluginSource::Remote,
                installed,
                enabled,
                install_policy,
                auth_policy: upstream::PluginAuthPolicy::OnUse,
                availability: upstream::PluginAvailability::default(),
                interface: display.map(|d| iface(d, "")),
                keywords: Vec::new(),
            }
        }

        fn marketplace(
            name: &str,
            plugins: Vec<upstream::PluginSummary>,
        ) -> upstream::PluginMarketplaceEntry {
            upstream::PluginMarketplaceEntry {
                name: name.into(),
                path: Some(AbsolutePathBuf::try_from("/tmp/marketplace.json").unwrap()),
                interface: None,
                plugins,
            }
        }

        fn response(
            marketplaces: Vec<upstream::PluginMarketplaceEntry>,
        ) -> upstream::PluginListResponse {
            upstream::PluginListResponse {
                marketplaces,
                marketplace_load_errors: Vec::new(),
                featured_plugin_ids: Vec::new(),
            }
        }

        #[test]
        fn flattens_marketplaces_and_attaches_marketplace_name() {
            let response = response(vec![
                marketplace(
                    "openai-curated",
                    vec![summary(
                        "p1",
                        "computer-use",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Computer Use"),
                    )],
                ),
                marketplace(
                    "community",
                    vec![summary(
                        "p2",
                        "linear",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Linear"),
                    )],
                ),
            ]);

            let shaped = shape_plugin_list(response);
            assert_eq!(shaped.len(), 2);
            assert_eq!(shaped[0].name, "computer-use");
            assert_eq!(shaped[0].marketplace_name, "openai-curated");
            assert_eq!(
                shaped[0].mention_path,
                "plugin://computer-use@openai-curated"
            );
            assert_eq!(shaped[1].name, "linear");
            assert_eq!(shaped[1].marketplace_name, "community");
        }

        #[test]
        fn filters_unavailable_plugins() {
            let response = response(vec![marketplace(
                "openai-curated",
                vec![
                    summary(
                        "skip",
                        "not-installed",
                        false,
                        false,
                        upstream::PluginInstallPolicy::Available,
                        None,
                    ),
                    summary(
                        "keep-installed",
                        "alpha",
                        true,
                        false,
                        upstream::PluginInstallPolicy::Available,
                        None,
                    ),
                    summary(
                        "keep-default",
                        "beta",
                        false,
                        false,
                        upstream::PluginInstallPolicy::InstalledByDefault,
                        None,
                    ),
                    summary(
                        "keep-enabled",
                        "gamma",
                        false,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        None,
                    ),
                ],
            )]);

            let shaped = shape_plugin_list(response);
            let names: Vec<&str> = shaped.iter().map(|s| s.name.as_str()).collect();
            assert_eq!(names, vec!["alpha", "beta", "gamma"]);
        }

        #[test]
        fn dedupes_by_mention_path() {
            let response = response(vec![
                marketplace(
                    "openai-curated",
                    vec![summary(
                        "first",
                        "computer-use",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Computer Use (first)"),
                    )],
                ),
                marketplace(
                    "openai-curated",
                    vec![summary(
                        "second",
                        "computer-use",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Computer Use (second)"),
                    )],
                ),
            ]);

            let shaped = shape_plugin_list(response);
            assert_eq!(shaped.len(), 1);
            assert_eq!(shaped[0].id, "first");
        }

        #[test]
        fn sorts_by_display_title_case_insensitive() {
            let response = response(vec![marketplace(
                "openai-curated",
                vec![
                    summary(
                        "z",
                        "zeta",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("zeta"),
                    ),
                    summary(
                        "a",
                        "alpha",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Alpha"),
                    ),
                    summary(
                        "m",
                        "mike",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        Some("Mike"),
                    ),
                ],
            )]);

            let shaped = shape_plugin_list(response);
            let titles: Vec<&str> = shaped.iter().map(|s| s.display_title.as_str()).collect();
            assert_eq!(titles, vec!["Alpha", "Mike", "zeta"]);
        }

        #[test]
        fn falls_back_to_name_when_display_name_blank() {
            let response = response(vec![marketplace(
                "openai-curated",
                vec![upstream::PluginSummary {
                    interface: Some(iface("   ", "")),
                    ..summary(
                        "p",
                        "linear",
                        true,
                        true,
                        upstream::PluginInstallPolicy::Available,
                        None,
                    )
                }],
            )]);

            let shaped = shape_plugin_list(response);
            assert_eq!(shaped[0].display_title, "linear");
        }
    }
}

#[cfg(test)]
#[path = "saved_app_tests.rs"]
mod saved_app_remote_tests;
