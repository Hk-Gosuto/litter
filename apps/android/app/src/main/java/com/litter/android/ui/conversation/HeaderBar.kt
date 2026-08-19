package com.litter.android.ui.conversation

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigkitten.litter.android.R
import com.litter.android.state.accentColor
import com.litter.android.state.displayModelLabel
import com.litter.android.state.resolvedModel
import com.litter.android.state.statusColor
import com.litter.android.ui.LitterTextStyle
import com.litter.android.ui.LocalAppModel
import com.litter.android.ui.LitterTheme
import com.litter.android.ui.LocalAppModel
import kotlinx.coroutines.launch
import uniffi.codex_mobile_client.AppThreadSnapshot

/** Floating conversation navigation; model and mode controls live in the composer. */
@Composable
fun HeaderBar(
    thread: AppThreadSnapshot?,
    onBack: () -> Unit,
    onInfo: (() -> Unit)? = null,
    onReloadError: ((String) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    val snapshot by appModel.snapshot.collectAsState()
    val scope = rememberCoroutineScope()
    val server = remember(snapshot, thread) {
        thread?.let { current -> snapshot?.servers?.find { it.serverId == current.key.serverId } }
    }
    var isReloading by remember { mutableStateOf(false) }

    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FloatingHeaderButton(onClick = onBack) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = "Back",
                tint = LitterTheme.textPrimary,
                modifier = Modifier.size(20.dp),
            )
        }

        Spacer(Modifier.weight(1f))

        FloatingHeaderButton(
            enabled = !isReloading && thread != null,
            onClick = {
                val currentThread = thread ?: return@FloatingHeaderButton
                scope.launch {
                    isReloading = true
                    try {
                        if (server?.requiresOpenaiAuth == true && server.account == null) {
                            val authUrl = appModel.client.startRemoteSshOauthLogin(currentThread.key.serverId)
                            CustomTabsIntent.Builder().setShowTitle(true).build()
                                .launchUrl(context, Uri.parse(authUrl))
                        } else {
                            val nextKey = appModel.refreshThreadIncludingTurns(currentThread.key)
                            appModel.store.setActiveThread(nextKey)
                        }
                    } catch (error: Exception) {
                        onReloadError?.invoke(error.message ?: "Failed to reload conversation")
                    } finally {
                        isReloading = false
                    }
                }
            },
        ) {
            if (isReloading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                    color = LitterTheme.accent,
                )
            } else {
                Icon(
                    Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = stringResource(R.string.common_back),
                    tint = LitterTheme.textPrimary,
                    modifier = Modifier.size(20.dp),
                )
            }

            // Status dot
            val health = server?.health ?: AppServerHealth.UNKNOWN
            val statusColor = server?.statusColor ?: health.accentColor
            val shouldPulse = health == AppServerHealth.CONNECTING || health == AppServerHealth.UNRESPONSIVE
            val dotAlpha = if (shouldPulse) {
                val infiniteTransition = rememberInfiniteTransition(label = "statusDotPulse")
                infiniteTransition.animateFloat(
                    initialValue = 0.3f,
                    targetValue = 1.0f,
                    animationSpec = infiniteRepeatable(
                        animation = tween(durationMillis = 1000),
                        repeatMode = RepeatMode.Reverse,
                    ),
                    label = "statusDotAlpha",
                ).value
            } else {
                1.0f
            }
            Box(
                modifier = Modifier
                    .size(8.dp)
                    .clip(CircleShape)
                    .background(statusColor.copy(alpha = dotAlpha)),
            )
            Spacer(Modifier.width(8.dp))

            // Model + reasoning label (tappable)
            Column(
                modifier = Modifier
                    .weight(1f)
                    .clickable { onToggleModelSelector() },
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = modelLabel,
                        color = LitterTheme.textPrimary,
                        fontSize = LitterTextStyle.footnote.scaled,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (HeaderOverrides.pendingFastMode) {
                        Spacer(Modifier.width(4.dp))
                        Text(
                            text = "\u26A1",
                            color = LitterTheme.warning,
                            fontSize = LitterTextStyle.caption2.scaled,
                        )
                    }
                    if (reasoningLabel.isNotBlank()) {
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = reasoningLabel,
                            color = LitterTheme.textSecondary,
                            fontSize = LitterTextStyle.caption.scaled,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Spacer(Modifier.width(2.dp))
                    Icon(
                        Icons.Default.KeyboardArrowDown,
                        contentDescription = stringResource(R.string.conversation_open_model_selector),
                        tint = LitterTheme.textSecondary,
                        modifier = Modifier.size(14.dp),
                    )
                }
                val cwd = thread?.info?.cwd
                if (cwd != null) {
                    val abbreviated = cwd.replace(Regex("^/home/[^/]+"), "~")
                        .replace(Regex("^/Users/[^/]+"), "~")
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = abbreviated,
                            color = LitterTheme.textMuted,
                            fontSize = 10f.scaled,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        if (thread?.collaborationMode == AppModeKind.PLAN) {
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = stringResource(R.string.conversation_plan),
                                color = Color.Black,
                                fontSize = 10f.scaled,
                                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                                modifier = Modifier
                                    .background(
                                        LitterTheme.accent,
                                        RoundedCornerShape(999.dp),
                                    )
                                    .padding(horizontal = 6.dp, vertical = 2.dp),
                            )
                        }
                        // Permission-preset chip: show a danger-tinted `lock.open`
                        // when this thread is running at full-access (approval =
                        // never + sandbox = danger-full-access). Mirrors iOS
                        // HeaderView.swift:74-78 `headerPermissionPreset == .fullAccess`.
                        val permissionPreset = thread
                            ?.takeIf { it.agentRuntimeKind.reportsEffectiveThreadPermissions }
                            ?.let { t ->
                                val approval = appModel.launchState.approvalPolicyValue(t.key)
                                    ?: t.effectiveApprovalPolicy
                                val sandbox = appModel.launchState.turnSandboxPolicy(t.key)
                                    ?: t.effectiveSandboxPolicy
                                uniffi.codex_mobile_client.threadPermissionPreset(approval, sandbox)
                            }
                        if (permissionPreset ==
                            uniffi.codex_mobile_client.AppThreadPermissionPreset.FULL_ACCESS) {
                            Spacer(Modifier.width(6.dp))
                            Icon(
                                imageVector = Icons.Default.LockOpen,
                                contentDescription = stringResource(R.string.conversation_full_access_permissions),
                                tint = LitterTheme.danger,
                                modifier = Modifier.size(10.dp),
                            )
                        }
                    }
                }
            }

            // Reload button
            var isReloading by remember { mutableStateOf(false) }
            IconButton(
                onClick = {
                    if (thread == null || isReloading) return@IconButton
                    scope.launch {
                        isReloading = true
                        try {
                            if (server?.requiresOpenaiAuth == true && server.account == null) {
                                val authUrl = appModel.client.startRemoteSshOauthLogin(
                                    thread.key.serverId,
                                )
                                CustomTabsIntent.Builder()
                                    .setShowTitle(true)
                                    .build()
                                    .launchUrl(context, Uri.parse(authUrl))
                                return@launch
                            }
                            val nextKey = appModel.refreshThreadIncludingTurns(thread.key)
                            appModel.store.setActiveThread(nextKey)
                        } catch (e: Exception) {
                            onReloadError?.invoke(e.message ?: "Failed to reload conversation")
                        } finally {
                            isReloading = false
                        }
                    }
                },
                enabled = !isReloading,
                modifier = Modifier.size(32.dp),
            ) {
                if (isReloading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        strokeWidth = 2.dp,
                        color = LitterTheme.accent,
                    )
                } else {
                    Icon(
                        Icons.Default.Refresh,
                        contentDescription = stringResource(R.string.conversation_reload),
                        tint = LitterTheme.textSecondary,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }

            // Info button
            if (onInfo != null) {
                IconButton(
                    onClick = onInfo,
                    modifier = Modifier.size(32.dp),
                ) {
                    Icon(
                        Icons.Outlined.Info,
                        contentDescription = stringResource(R.string.conversation_info),
                        tint = LitterTheme.textSecondary,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }

        if (onInfo != null) {
            FloatingHeaderButton(onClick = onInfo) {
                Icon(
                    Icons.Outlined.Info,
                    contentDescription = "Info",
                    tint = LitterTheme.textSecondary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

@Composable
private fun FloatingHeaderButton(
    onClick: () -> Unit,
    enabled: Boolean = true,
    content: @Composable () -> Unit,
) {
    IconButton(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier
            .size(40.dp)
            .clip(CircleShape)
            .background(LitterTheme.surface.copy(alpha = 0.86f)),
        content = content,
    )
}

/** Shared fast-tier override selected from the model options sheet. */
object HeaderOverrides {
    var pendingFastMode by mutableStateOf(false)
}
