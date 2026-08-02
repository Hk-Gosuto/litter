package com.litter.android.ui.conversation

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigkitten.litter.android.R
import com.litter.android.ui.LocalAppModel
import com.litter.android.ui.LitterTextStyle
import com.litter.android.ui.LitterTheme
import com.litter.android.ui.common.reportsEffectiveThreadPermissions
import com.litter.android.ui.common.supportsThreadPermissionOverrides
import com.litter.android.ui.scaled
import kotlinx.coroutines.launch
import uniffi.codex_mobile_client.AbsolutePath
import uniffi.codex_mobile_client.AppAskForApproval
import uniffi.codex_mobile_client.AppListExperimentalFeaturesRequest
import uniffi.codex_mobile_client.AppListSkillsRequest
import uniffi.codex_mobile_client.AppWriteConfigValueRequest
import uniffi.codex_mobile_client.ExperimentalFeature
import uniffi.codex_mobile_client.AppMergeStrategy
import uniffi.codex_mobile_client.AppSandboxPolicy
import uniffi.codex_mobile_client.SkillMetadata
import uniffi.codex_mobile_client.ThreadKey
import uniffi.codex_mobile_client.threadPermissionsAreAuthoritative

private data class ComposerSelectionOption(
    val titleRes: Int,
    val descriptionRes: Int,
    val wireValue: String,
)

private val composerApprovalOptions = listOf(
    ComposerSelectionOption(
        titleRes = R.string.permissions_default,
        descriptionRes = R.string.permissions_default_detail,
        wireValue = "inherit",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_untrusted,
        descriptionRes = R.string.permissions_untrusted_detail,
        wireValue = "untrusted",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_on_request,
        descriptionRes = R.string.permissions_on_request_detail,
        wireValue = "on-request",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_never,
        descriptionRes = R.string.permissions_never_detail,
        wireValue = "never",
    ),
)

private val composerSandboxOptions = listOf(
    ComposerSelectionOption(
        titleRes = R.string.permissions_default,
        descriptionRes = R.string.permissions_default_detail,
        wireValue = "inherit",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_read_only,
        descriptionRes = R.string.permissions_read_only_detail,
        wireValue = "read-only",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_workspace_write,
        descriptionRes = R.string.permissions_workspace_write_detail,
        wireValue = "workspace-write",
    ),
    ComposerSelectionOption(
        titleRes = R.string.permissions_full_access,
        descriptionRes = R.string.permissions_full_access_detail,
        wireValue = "danger-full-access",
    ),
)

@Composable
private fun selectedApprovalLabel(approvalPolicy: String): String =
    composerApprovalOptions.firstOrNull { it.wireValue == approvalPolicy }
        ?.let { stringResource(it.titleRes) }
        ?: stringResource(R.string.permissions_custom)

@Composable
private fun selectedSandboxLabel(sandboxMode: String): String =
    composerSandboxOptions.firstOrNull { it.wireValue == sandboxMode }
        ?.let { stringResource(it.titleRes) }
        ?: stringResource(R.string.permissions_custom)

@Composable
private fun AppAskForApproval.displayTitle(): String =
    when (this) {
        AppAskForApproval.UnlessTrusted -> stringResource(R.string.permissions_untrusted)
        AppAskForApproval.OnRequest -> stringResource(R.string.permissions_on_request)
        is AppAskForApproval.Granular -> stringResource(R.string.permissions_granular)
        AppAskForApproval.Never -> stringResource(R.string.permissions_never)
    }

@Composable
private fun AppSandboxPolicy.displayTitle(): String =
    when (this) {
        AppSandboxPolicy.DangerFullAccess -> stringResource(R.string.permissions_full_access)
        is AppSandboxPolicy.ReadOnly -> stringResource(R.string.permissions_read_only)
        is AppSandboxPolicy.WorkspaceWrite -> stringResource(R.string.permissions_workspace_write)
        is AppSandboxPolicy.ExternalSandbox -> stringResource(R.string.permissions_external_sandbox)
    }

@Composable
fun ComposerPermissionsSheet(threadKey: ThreadKey? = null, onDismiss: () -> Unit) {
    val appModel = LocalAppModel.current
    val launchState by appModel.launchState.snapshot.collectAsState()
    val selectedApproval = appModel.launchState.selectedApprovalPolicy(threadKey)
    val selectedSandbox = appModel.launchState.selectedSandboxMode(threadKey)
    val effectiveThread = appModel.snapshot.value?.threads?.firstOrNull { it.key == threadKey }
    val selectedRuntime = effectiveThread?.agentRuntimeKind ?: launchState.selectedAgentRuntimeKind
    val currentRuntimeSupportsPermissionOverrides =
        selectedRuntime?.supportsThreadPermissionOverrides ?: true
    val hasAuthoritativeThreadPermissions = if (
        (selectedRuntime?.reportsEffectiveThreadPermissions ?: true) &&
        threadPermissionsAreAuthoritative(
            approvalPolicy = effectiveThread?.effectiveApprovalPolicy,
            sandboxPolicy = effectiveThread?.effectiveSandboxPolicy,
        )
    ) true else false
    val currentApprovalLabel =
        if (hasAuthoritativeThreadPermissions) {
            effectiveThread?.effectiveApprovalPolicy?.displayTitle()
                ?: stringResource(R.string.permissions_syncing)
        } else {
            stringResource(R.string.permissions_syncing)
        }
    val currentSandboxLabel =
        if (hasAuthoritativeThreadPermissions) {
            effectiveThread?.effectiveSandboxPolicy?.displayTitle()
                ?: stringResource(R.string.permissions_syncing)
        } else {
            stringResource(R.string.permissions_syncing)
        }
    val usesThreadDefaults = selectedApproval == "inherit" && selectedSandbox == "inherit"
    val scrollState = rememberScrollState()

    LaunchedEffect(threadKey) {
        threadKey ?: return@LaunchedEffect
        appModel.hydrateThreadPermissions(threadKey)
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .fillMaxSize(fraction = 0.92f)
            .verticalScroll(scrollState)
            .imePadding()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SheetHeader(title = stringResource(R.string.permissions_title), onDismiss = onDismiss)
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(LitterTheme.surface.copy(alpha = 0.82f), RoundedCornerShape(20.dp))
                .border(1.dp, LitterTheme.border.copy(alpha = 0.55f), RoundedCornerShape(20.dp))
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(
                        text = stringResource(R.string.permissions_thread),
                        color = LitterTheme.textPrimary,
                        fontSize = 18f.scaled,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = if (currentRuntimeSupportsPermissionOverrides) {
                            stringResource(R.string.permissions_next_turn_detail)
                        } else {
                            stringResource(R.string.permissions_runtime_controls)
                        },
                        color = LitterTheme.textMuted,
                        fontSize = LitterTextStyle.caption2.scaled,
                    )
                }
                Text(
                    text = if (!currentRuntimeSupportsPermissionOverrides) {
                        stringResource(R.string.permissions_runtime_managed)
                    } else if (usesThreadDefaults) {
                        stringResource(R.string.permissions_using_defaults)
                    } else {
                        stringResource(R.string.permissions_custom_override)
                    },
                    color = if (!currentRuntimeSupportsPermissionOverrides || usesThreadDefaults) {
                        LitterTheme.textSecondary
                    } else {
                        LitterTheme.accentStrong
                    },
                    fontSize = LitterTextStyle.caption2.scaled,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier
                        .background(
                            color = (
                                if (!currentRuntimeSupportsPermissionOverrides || usesThreadDefaults) {
                                    LitterTheme.surfaceLight
                                } else {
                                    LitterTheme.accentStrong
                                }
                            ).copy(alpha = 0.16f),
                            shape = RoundedCornerShape(999.dp),
                        )
                        .padding(horizontal = 10.dp, vertical = 6.dp),
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                PermissionSummaryTile(
                    title = stringResource(R.string.permissions_next_turn),
                    approval = selectedApprovalLabel(selectedApproval),
                    sandbox = selectedSandboxLabel(selectedSandbox),
                    accent = LitterTheme.accentStrong,
                    modifier = Modifier.weight(1f),
                )
                if (threadKey != null) {
                    PermissionSummaryTile(
                        title = stringResource(R.string.permissions_current_thread),
                        approval = currentApprovalLabel,
                        sandbox = currentSandboxLabel,
                        accent = if (hasAuthoritativeThreadPermissions) LitterTheme.textSecondary else LitterTheme.warning,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
        }

        if (currentRuntimeSupportsPermissionOverrides) {
            PermissionSettingsSection(
                title = stringResource(R.string.permissions_approval_policy),
                subtitle = stringResource(R.string.permissions_approval_helper),
            ) {
                PermissionDropdownField(
                    options = composerApprovalOptions,
                    selectedValue = selectedApproval,
                    onSelect = { value ->
                        appModel.launchState.updateThreadPermissions(threadKey, value, selectedSandbox)
                    },
                )
            }

            PermissionSettingsSection(
                title = stringResource(R.string.permissions_sandbox_settings),
                subtitle = stringResource(R.string.permissions_sandbox_helper),
            ) {
                PermissionDropdownField(
                    options = composerSandboxOptions,
                    selectedValue = selectedSandbox,
                    onSelect = { value ->
                        appModel.launchState.updateThreadPermissions(threadKey, selectedApproval, value)
                    },
                )
            }
        } else {
            PermissionSettingsSection(
                title = stringResource(R.string.permissions_runtime_managed),
                subtitle = stringResource(R.string.permissions_runtime_override_unavailable),
            ) {
                Text(
                    text = stringResource(R.string.permissions_runtime_own_controls),
                    color = LitterTheme.textSecondary,
                    fontSize = LitterTextStyle.caption.scaled,
                )
            }
        }
    }
}

@Composable
private fun PermissionSummaryTile(
    title: String,
    approval: String,
    sandbox: String,
    accent: androidx.compose.ui.graphics.Color,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .background(LitterTheme.surfaceLight.copy(alpha = 0.78f), RoundedCornerShape(16.dp))
            .border(1.dp, LitterTheme.border.copy(alpha = 0.45f), RoundedCornerShape(16.dp))
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = title,
            color = LitterTheme.textSecondary,
            fontSize = LitterTextStyle.caption2.scaled,
            fontWeight = FontWeight.SemiBold,
        )
        PermissionSummaryRow(label = stringResource(R.string.permissions_approval_summary), value = approval, accent = accent)
        PermissionSummaryRow(label = stringResource(R.string.permissions_sandbox_summary), value = sandbox, accent = accent)
    }
}

@Composable
private fun PermissionSummaryRow(
    label: String,
    value: String,
    accent: androidx.compose.ui.graphics.Color,
) {
    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(
            text = label,
            color = LitterTheme.textMuted,
            fontSize = 10f.scaled,
            fontWeight = FontWeight.Medium,
        )
        Text(
            text = value,
            color = accent,
            fontSize = LitterTextStyle.code.scaled,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

@Composable
private fun PermissionSettingsSection(
    title: String,
    subtitle: String,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(LitterTheme.surface.copy(alpha = 0.74f), RoundedCornerShape(20.dp))
            .border(1.dp, LitterTheme.border.copy(alpha = 0.5f), RoundedCornerShape(20.dp))
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                text = title,
                color = LitterTheme.textPrimary,
                fontSize = 18f.scaled,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = subtitle,
                color = LitterTheme.textSecondary,
                fontSize = LitterTextStyle.caption.scaled,
            )
        }
        content()
    }
}

@Composable
private fun PermissionDropdownField(
    options: List<ComposerSelectionOption>,
    selectedValue: String,
    onSelect: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedOption = options.firstOrNull { it.wireValue == selectedValue }

    Box(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(LitterTheme.surfaceLight.copy(alpha = 0.9f), RoundedCornerShape(14.dp))
                .border(1.dp, LitterTheme.border.copy(alpha = 0.45f), RoundedCornerShape(14.dp))
                .clickable { expanded = true }
                .padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    text = selectedOption?.let { stringResource(it.titleRes) }
                        ?: stringResource(R.string.permissions_custom),
                    color = LitterTheme.textPrimary,
                    fontSize = LitterTextStyle.body.scaled,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                )
                Text(
                    text = selectedOption?.let { stringResource(it.descriptionRes) }
                        ?: stringResource(R.string.permissions_managed_server),
                    color = LitterTheme.textMuted,
                    fontSize = LitterTextStyle.caption2.scaled,
                    maxLines = 1,
                )
            }
            Icon(
                imageVector = Icons.Default.KeyboardArrowDown,
                contentDescription = null,
                tint = LitterTheme.textMuted,
                modifier = Modifier.size(18.dp),
            )
        }

        DropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
        ) {
            options.forEach { option ->
                DropdownMenuItem(
                    text = {
                        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(
                                text = stringResource(option.titleRes),
                                color = LitterTheme.textPrimary,
                                fontSize = LitterTextStyle.body.scaled,
                                fontWeight = FontWeight.SemiBold,
                            )
                            Text(
                                text = stringResource(option.descriptionRes),
                                color = LitterTheme.textMuted,
                                fontSize = LitterTextStyle.caption2.scaled,
                            )
                        }
                    },
                    trailingIcon = {
                        if (selectedValue == option.wireValue) {
                            Icon(
                                imageVector = Icons.Default.Check,
                                contentDescription = null,
                                tint = LitterTheme.accentStrong,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    },
                    onClick = {
                        expanded = false
                        onSelect(option.wireValue)
                    },
                )
            }
        }
    }
}

@Composable
fun ComposerExperimentalSheet(
    serverId: String,
    onDismiss: () -> Unit,
    onError: (String) -> Unit,
) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var features by remember(serverId) { mutableStateOf<List<ExperimentalFeature>>(emptyList()) }
    var isLoading by remember(serverId) { mutableStateOf(true) }
    var reloadToken by remember(serverId) { mutableIntStateOf(0) }

    LaunchedEffect(serverId, reloadToken) {
        isLoading = true
        runCatching {
            appModel.client.listExperimentalFeatures(
                serverId,
                AppListExperimentalFeaturesRequest(cursor = null, limit = 200u),
            )
        }.onSuccess { featuresResult ->
            features = featuresResult.sortedBy { (it.displayName ?: it.name).lowercase() }
        }.onFailure { error ->
            features = emptyList()
            onError(error.message ?: context.getString(R.string.experimental_load_failed))
        }
        isLoading = false
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .fillMaxSize(fraction = 0.9f)
            .imePadding()
            .padding(16.dp),
    ) {
        SheetHeader(
            title = stringResource(R.string.settings_experimental_section),
            leadingActionLabel = stringResource(R.string.common_reload),
            onLeadingAction = { reloadToken += 1 },
            onDismiss = onDismiss,
        )
        Spacer(Modifier.height(12.dp))
        when {
            isLoading -> {
                Box(Modifier.fillMaxWidth().padding(vertical = 32.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(color = LitterTheme.accent, modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                }
            }

            features.isEmpty() -> {
                Text(stringResource(R.string.conversation_no_experimental_features), color = LitterTheme.textMuted, fontSize = LitterTextStyle.code.scaled)
            }

            else -> {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(features, key = { it.name }) { feature ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface.copy(alpha = 0.72f), RoundedCornerShape(12.dp))
                                .padding(horizontal = 14.dp, vertical = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Text(feature.displayName ?: feature.name, color = LitterTheme.textPrimary, fontSize = LitterTextStyle.body.scaled)
                                feature.description?.takeIf { it.isNotBlank() }?.let { description ->
                                    Text(description, color = LitterTheme.textSecondary, fontSize = LitterTextStyle.caption.scaled)
                                }
                            }
                            Switch(
                                checked = feature.enabled,
                                onCheckedChange = { enabled ->
                                    val previous = feature.enabled
                                    features = features.map {
                                        if (it.name == feature.name) {
                                            ExperimentalFeature(
                                                name = it.name,
                                                stage = it.stage,
                                                displayName = it.displayName,
                                                description = it.description,
                                                announcement = it.announcement,
                                                enabled = enabled,
                                                defaultEnabled = it.defaultEnabled,
                                            )
                                        } else {
                                            it
                                        }
                                    }
                                    scope.launch {
                                        runCatching {
                                            appModel.client.writeConfigValue(
                                                serverId,
                                                AppWriteConfigValueRequest(
                                                    keyPath = "features.${feature.name}",
                                                    valueJson = if (enabled) "true" else "false",
                                                    mergeStrategy = AppMergeStrategy.UPSERT,
                                                    filePath = null,
                                                    expectedVersion = null,
                                                ),
                                            )
                                        }.onFailure { error ->
                                            features = features.map {
                                                if (it.name == feature.name) {
                                                    ExperimentalFeature(
                                                        name = it.name,
                                                        stage = it.stage,
                                                        displayName = it.displayName,
                                                        description = it.description,
                                                        announcement = it.announcement,
                                                        enabled = previous,
                                                        defaultEnabled = it.defaultEnabled,
                                                    )
                                                } else {
                                                    it
                                                }
                                            }
                                            onError(
                                                error.message
                                                    ?: context.getString(R.string.experimental_update_failed),
                                            )
                                        }
                                    }
                                },
                                colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accent),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun ComposerSkillsSheet(
    serverId: String,
    cwd: String,
    onDismiss: () -> Unit,
    onError: (String) -> Unit,
) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    var skills by remember(serverId, cwd) { mutableStateOf<List<SkillMetadata>>(emptyList()) }
    var isLoading by remember(serverId, cwd) { mutableStateOf(true) }
    var reloadToken by remember(serverId, cwd) { mutableIntStateOf(0) }

    LaunchedEffect(serverId, cwd, reloadToken) {
        isLoading = true
        runCatching {
            appModel.client.listSkills(
                serverId,
                AppListSkillsRequest(
                    cwds = listOf(cwd),
                    forceReload = reloadToken > 0,
                ),
            )
        }.onSuccess { skillResults ->
            skills = skillResults.sortedBy { it.name.lowercase() }
        }.onFailure { error ->
            skills = emptyList()
            onError(error.message ?: context.getString(R.string.skills_load_failed))
        }
        isLoading = false
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .fillMaxSize(fraction = 0.9f)
            .imePadding()
            .padding(16.dp),
    ) {
        SheetHeader(
            title = stringResource(R.string.composer_skills),
            leadingActionLabel = stringResource(R.string.common_reload),
            onLeadingAction = { reloadToken += 1 },
            onDismiss = onDismiss,
        )
        Spacer(Modifier.height(12.dp))
        when {
            isLoading -> {
                Box(Modifier.fillMaxWidth().padding(vertical = 32.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(color = LitterTheme.accent, modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                }
            }

            skills.isEmpty() -> {
                Text(stringResource(R.string.conversation_no_skills), color = LitterTheme.textMuted, fontSize = LitterTextStyle.code.scaled)
            }

            else -> {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(skills, key = { "${it.path.value}#${it.name}" }) { skill ->
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface.copy(alpha = 0.72f), RoundedCornerShape(12.dp))
                                .padding(horizontal = 14.dp, vertical = 12.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(skill.name, color = LitterTheme.textPrimary, fontSize = LitterTextStyle.body.scaled, fontWeight = FontWeight.Medium)
                                Spacer(Modifier.weight(1f))
                                if (skill.enabled) {
                                    Text(
                                        "enabled",
                                        color = LitterTheme.accent,
                                        fontSize = LitterTextStyle.caption2.scaled,
                                        modifier = Modifier
                                            .background(LitterTheme.accent.copy(alpha = 0.14f), RoundedCornerShape(999.dp))
                                            .padding(horizontal = 6.dp, vertical = 2.dp),
                                    )
                                }
                            }
                            Text(skill.description, color = LitterTheme.textSecondary, fontSize = LitterTextStyle.caption.scaled)
                            Text(skill.path.value, color = LitterTheme.textMuted, fontSize = LitterTextStyle.caption2.scaled)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SheetHeader(
    title: String,
    leadingActionLabel: String? = null,
    onLeadingAction: (() -> Unit)? = null,
    onDismiss: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (leadingActionLabel != null && onLeadingAction != null) {
            TextButton(onClick = onLeadingAction) {
                Text(leadingActionLabel, color = LitterTheme.accent)
            }
        } else {
            Spacer(Modifier.width(64.dp))
        }
        Spacer(Modifier.weight(1f))
        Text(title, color = LitterTheme.textPrimary, fontSize = LitterTextStyle.headline.scaled, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.weight(1f))
        TextButton(onClick = onDismiss) {
            Text(stringResource(R.string.settings_done), color = LitterTheme.accent)
        }
    }
    HorizontalDivider(color = LitterTheme.divider, modifier = Modifier.padding(top = 8.dp))
}
