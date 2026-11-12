package com.litter.android.ui.settings

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Pets
import androidx.compose.material.icons.filled.Language
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material.icons.filled.Science
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Widgets
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.litter.android.auth.ChatGPTOAuthActivity
import com.sigkitten.litter.android.R
import com.litter.android.state.ChatGPTOAuth
import com.litter.android.state.ChatGPTOAuthTokenStore
import com.litter.android.state.DebugSettings
import com.litter.android.state.MessageRecorder
import com.litter.android.state.OpenAIApiKeyStore
import com.litter.android.state.PetOverlayController
import com.litter.android.state.SavedServer
import com.litter.android.state.SavedServerStore
import com.litter.android.state.SshAuthMethod
import com.litter.android.state.SshCredentialStore
import com.litter.android.state.connectionModeLabel
import com.litter.android.state.isConnected
import com.litter.android.state.statusColor
import com.litter.android.state.statusLabel
import com.litter.android.state.toRecord
import com.litter.android.ui.LocalAppModel
import com.litter.android.ui.AppLanguage
import com.litter.android.ui.AppLanguagePreferences
import com.litter.android.ui.LitterAppearanceMode
import com.litter.android.ui.LitterColorThemeType
import com.litter.android.ui.BerkeleyMono
import com.litter.android.ui.ConversationPrefs
import com.litter.android.ui.ExperimentalFeatures
import com.litter.android.ui.LitterFeature
import com.litter.android.ui.LitterFontFamilyOption
import com.litter.android.ui.WallpaperBackdrop
import com.litter.android.ui.WallpaperManager
import com.litter.android.ui.LitterTheme
import com.litter.android.ui.LitterThemeIndexEntry
import com.litter.android.ui.LitterThemeManager
import com.litter.android.ui.discovery.SSHLoginDialog
import com.litter.android.util.LLog
import kotlinx.coroutines.launch
import uniffi.codex_mobile_client.Account
import uniffi.codex_mobile_client.AppServerSnapshot
import uniffi.codex_mobile_client.AppLoginAccountRequest
import uniffi.codex_mobile_client.AppPetSummary

/**
 * Settings — hierarchical navigation matching iOS:
 * Top level: Appearance → | Font | Conversation | Experimental → | Account | Servers
 * Appearance pushes to sub-screen with theme pickers.
 * Experimental pushes to sub-screen with feature toggles.
 */

// ═══════════════════════════════════════════════════════════════════════════════
// Top-level Settings
// ═══════════════════════════════════════════════════════════════════════════════

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsSheet(
    onDismiss: () -> Unit,
    onOpenAccount: (serverId: String) -> Unit,
    initialSubScreen: SettingsStartDestination = SettingsStartDestination.TopLevel,
    onOpenApps: (() -> Unit)? = null,
) {
    // Sub-screen navigation
    var subScreen by remember(initialSubScreen) {
        mutableStateOf(
            when (initialSubScreen) {
                SettingsStartDestination.TopLevel -> null
                SettingsStartDestination.Pets -> SettingsSubScreen.Pets
            },
        )
    }

    when (subScreen) {
        SettingsSubScreen.Harnesses -> HarnessSettingsScreen(onBack = { subScreen = null })
        SettingsSubScreen.Appearance -> AppearanceScreen(onBack = { subScreen = null })
        SettingsSubScreen.Experimental -> ExperimentalScreen(onBack = { subScreen = null })
        SettingsSubScreen.Pets -> PetsScreen(onBack = { subScreen = null })
        SettingsSubScreen.TipJar -> TipJarScreen(onBack = { subScreen = null })
        SettingsSubScreen.Debug -> DebugScreen(onBack = { subScreen = null })
        null -> SettingsTopLevel(
            onDismiss = onDismiss,
            onOpenHarnesses = { subScreen = SettingsSubScreen.Harnesses },
            onOpenAppearance = { subScreen = SettingsSubScreen.Appearance },
            onOpenExperimental = { subScreen = SettingsSubScreen.Experimental },
            onOpenPets = { subScreen = SettingsSubScreen.Pets },
            onOpenTipJar = { subScreen = SettingsSubScreen.TipJar },
            onOpenDebug = { subScreen = SettingsSubScreen.Debug },
            onOpenAccount = onOpenAccount,
            onOpenApps = onOpenApps,
        )
    }
}

enum class SettingsStartDestination { TopLevel, Pets }

private enum class SettingsSubScreen { Harnesses, Appearance, Experimental, Pets, TipJar, Debug }

@Composable
private fun SettingsTopLevel(
    onDismiss: () -> Unit,
    onOpenHarnesses: () -> Unit,
    onOpenAppearance: () -> Unit,
    onOpenExperimental: () -> Unit,
    onOpenPets: () -> Unit,
    onOpenTipJar: () -> Unit,
    onOpenDebug: () -> Unit,
    onOpenAccount: (serverId: String) -> Unit,
    onOpenApps: (() -> Unit)?,
) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    val snapshot by appModel.snapshot.collectAsState()
    val scope = rememberCoroutineScope()
    val collapseTurns = ConversationPrefs.areTurnsCollapsed
    var renameTarget by remember { mutableStateOf<AppServerSnapshot?>(null) }
    var renameText by remember { mutableStateOf("") }
    var currentLanguage by remember { mutableStateOf(AppLanguagePreferences.current(context)) }
    var showLanguagePicker by remember { mutableStateOf(false) }

    val currentServer = remember(snapshot) {
        val activeServerId = snapshot?.activeThread?.serverId
        snapshot?.servers?.firstOrNull { it.serverId == activeServerId }
            ?: snapshot?.servers?.firstOrNull { it.isLocal }
            ?: snapshot?.servers?.firstOrNull()
    }

    var editTarget by remember { mutableStateOf<AppServerSnapshot?>(null) }
    var sshReconnectTarget by remember { mutableStateOf<SavedServer?>(null) }

    LazyColumn(
        modifier = Modifier
            .testTag("settings.content")
            .fillMaxWidth()
            .imePadding()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        // Title
        item {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.settings_title), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
                TextButton(onClick = onDismiss, modifier = Modifier.align(Alignment.CenterEnd)) {
                    Text(stringResource(R.string.settings_done), color = LitterTheme.accent)
                }
            }
            Spacer(Modifier.height(8.dp))
        }

        // ── Support ──
        item { SectionHeader(stringResource(R.string.settings_support_section)) }
        item {
            NavRow(icon = Icons.Default.Pets, label = stringResource(R.string.settings_tip_the_kitty), onClick = onOpenTipJar)
        }

        // ── Local AI ──
        item { SectionHeader(stringResource(R.string.settings_local_ai)) }
        item {
            SettingsRow(
                icon = {
                    Icon(
                        Icons.Default.Computer,
                        contentDescription = null,
                        tint = LitterTheme.accent,
                        modifier = Modifier.size(20.dp),
                    )
                },
                label = stringResource(R.string.settings_local_studio),
                subtitle = "localstudio.ai",
                trailing = {
                    Icon(
                        Icons.AutoMirrored.Filled.OpenInNew,
                        contentDescription = null,
                        tint = LitterTheme.textMuted,
                        modifier = Modifier.size(16.dp),
                    )
                },
                onClick = {
                    context.startActivity(
                        Intent(Intent.ACTION_VIEW, Uri.parse("https://localstudio.ai")),
                    )
                },
            )
        }

        // ── Theme ──
        item { SectionHeader(stringResource(R.string.settings_theme_section)) }
        item {
            NavRow(icon = Icons.Default.Palette, label = stringResource(R.string.settings_appearance), onClick = onOpenAppearance)
        }

        // ── Language ──
        item { SectionHeader(stringResource(R.string.settings_language_section)) }
        item {
            SettingsRow(
                icon = {
                    Icon(
                        Icons.Default.Language,
                        contentDescription = null,
                        tint = LitterTheme.accent,
                        modifier = Modifier.size(20.dp),
                    )
                },
                label = stringResource(R.string.settings_app_language),
                subtitle = currentLanguage.nativeDisplayName,
                trailing = {
                    Icon(
                        Icons.Default.ChevronRight,
                        contentDescription = null,
                        tint = LitterTheme.textMuted,
                        modifier = Modifier.size(16.dp),
                    )
                },
                onClick = { showLanguagePicker = true },
            )
        }

        // ── Font ──
        item { SectionHeader(stringResource(R.string.settings_font_section)) }
        item {
            Column(
                Modifier.fillMaxWidth().background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)),
            ) {
                LitterFontFamilyOption.entries.forEachIndexed { index, option ->
                    if (index > 0) {
                        HorizontalDivider(color = LitterTheme.divider)
                    }
                    FontRow(
                        option.displayName,
                        LitterTheme.fontFamily(option),
                        LitterThemeManager.selectedFontFamily == option,
                    ) { LitterThemeManager.applyFont(option) }
                }
            }
        }

        // ── Conversation ──
        item { SectionHeader(stringResource(R.string.settings_conversation_section)) }
        item {
            SettingsRow(
                icon = { Text("⊟", color = LitterTheme.accent, fontSize = 16.sp) },
                label = stringResource(R.string.settings_collapse_turns),
                subtitle = stringResource(R.string.settings_collapse_turns_description),
                trailing = {
                    Switch(
                        checked = collapseTurns,
                        onCheckedChange = { ConversationPrefs.setCollapseTurns(context, it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accent),
                    )
                },
            )
        }

        // ── Pets ──
        item { SectionHeader(stringResource(R.string.settings_pet_section)) }
        item {
            SettingsRow(
                icon = { Icon(Icons.Default.Pets, null, tint = LitterTheme.accent, modifier = Modifier.size(18.dp)) },
                label = stringResource(R.string.settings_wake_pet),
                subtitle = PetOverlayController.selectedPet?.displayName
                    ?: stringResource(R.string.settings_choose_pet),
                trailing = {
                    Switch(
                        checked = PetOverlayController.visible,
                        onCheckedChange = { PetOverlayController.setVisible(context, it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accent),
                    )
                },
                onClick = onOpenPets,
            )
        }

        // ── Apps ──
        if (onOpenApps != null) {
            item { SectionHeader(stringResource(R.string.settings_apps_section)) }
            item {
                NavRow(
                    icon = Icons.Default.Widgets,
                    label = stringResource(R.string.settings_saved_apps),
                    onClick = {
                        onDismiss()
                        onOpenApps()
                    },
                )
            }
        }

        // ── Experimental ──
        item { SectionHeader(stringResource(R.string.settings_experimental_section)) }
        item {
            NavRow(
                icon = Icons.Default.Science,
                label = stringResource(R.string.settings_experimental_features),
                onClick = onOpenExperimental,
            )
        }

        // ── Debug ──
        if (DebugSettings.enabled) {
            item { SectionHeader(stringResource(R.string.settings_debug_section)) }
            item {
                NavRow(
                    icon = Icons.Default.Science,
                    label = stringResource(R.string.settings_debug_settings),
                    onClick = onOpenDebug,
                )
            }
        }

        // ── Account ──
        item { SectionHeader(stringResource(R.string.settings_account_section)) }
        item {
            if (currentServer != null) {
                val accountStatus = when (val account = currentServer!!.account) {
                    is Account.Chatgpt -> account.email.ifEmpty {
                        context.getString(R.string.settings_chatgpt_account)
                    }
                    is Account.ApiKey -> stringResource(R.string.settings_openai_api_key)

                    null -> stringResource(R.string.settings_not_logged_in)
                }
                SettingsRow(
                    icon = { Text("@", color = LitterTheme.accent, fontSize = 16.sp, fontWeight = FontWeight.SemiBold) },
                    label = currentServer!!.displayName,
                    subtitle = accountStatus,
                    trailing = {
                        Icon(
                            Icons.Default.ChevronRight,
                            null,
                            tint = LitterTheme.textMuted,
                            modifier = Modifier.size(16.dp),
                        )
                    },
                    onClick = { onOpenAccount(currentServer!!.serverId) },
                )
            } else {
                SettingsRow(label = stringResource(R.string.settings_connect_server_first))
            }
        }

        item { NavRow(Icons.Default.Computer, stringResource(R.string.settings_harnesses), onOpenHarnesses) }

        // ── Servers ──
        item { SectionHeader(stringResource(R.string.settings_servers_section)) }
        val servers = snapshot?.servers ?: emptyList()
        if (servers.isEmpty()) {
            item { SettingsRow(label = stringResource(R.string.settings_no_servers_connected)) }
        } else {
            items(servers, key = { it.serverId }) { server ->
                ServerSettingsRow(
                    server = server,
                    onRename = {
                        renameText = server.displayName
                        renameTarget = server
                    },
                    onEdit = {
                        editTarget = server
                    },
                    onRemove = {
                        scope.launch {
                            SavedServerStore.remove(context, server.serverId)
                            appModel.sshSessionStore.close(server.serverId)
                            appModel.serverBridge.disconnectServer(server.serverId)
                            appModel.refreshSnapshot()
                        }
                    },
                )
            }
        }

        item { Spacer(Modifier.height(32.dp)) }
    }

    if (showLanguagePicker) {
        AlertDialog(
            onDismissRequest = { showLanguagePicker = false },
            title = { Text(stringResource(R.string.settings_choose_language)) },
            text = {
                Column {
                    AppLanguage.entries.forEach { language ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    currentLanguage = language
                                    AppLanguagePreferences.set(context, language)
                                    showLanguagePicker = false
                                    (context as? Activity)?.recreate()
                                }
                                .padding(vertical = 12.dp),
                        ) {
                            Text(
                                text = language.nativeDisplayName,
                                color = LitterTheme.textPrimary,
                                modifier = Modifier.weight(1f),
                            )
                            if (currentLanguage == language) {
                                Icon(
                                    Icons.Default.Check,
                                    contentDescription = null,
                                    tint = LitterTheme.accent,
                                )
                            }
                        }
                    }
                }
            },
            confirmButton = {},
            dismissButton = {
                TextButton(onClick = { showLanguagePicker = false }) {
                    Text(stringResource(R.string.directory_picker_cancel))
                }
            },
        )
    }

    renameTarget?.let { server ->
        AlertDialog(
            onDismissRequest = { renameTarget = null },
            title = { Text(stringResource(R.string.settings_rename_server)) },
            text = {
                OutlinedTextField(
                    value = renameText,
                    onValueChange = { renameText = it },
                    label = { Text(stringResource(R.string.settings_name)) },
                    singleLine = true,
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    val trimmed = renameText.trim()
                    if (trimmed.isEmpty()) return@TextButton
                    scope.launch {
                        SavedServerStore.rename(context, server.serverId, trimmed)
                        appModel.refreshSnapshot()
                    }
                    renameTarget = null
                }) {
                    Text(stringResource(R.string.settings_save))
                }
            },
            dismissButton = {
                TextButton(onClick = { renameTarget = null }) {
                    Text(stringResource(R.string.directory_picker_cancel))
                }
            },
        )
    }

    editTarget?.let { server ->
        ServerEditSheet(
            server = server,
            onDismiss = { editTarget = null },
            onSave = { editTarget = null },
            onTriggerSshReconnect = { saved ->
                editTarget = null
                sshReconnectTarget = saved
            },
        )
    }

    sshReconnectTarget?.let { saved ->
        val sshCredentialStore = remember(context) { SshCredentialStore(context.applicationContext) }
        val sshPort = saved.resolvedSshPort
        SSHLoginDialog(
            server = saved,
            initialCredential = sshCredentialStore.load(saved.hostname, sshPort),
            onDismiss = { sshReconnectTarget = null },
            onConnect = { credential, rememberCredentials ->
                try {
                    if (rememberCredentials) {
                        sshCredentialStore.save(saved.hostname, sshPort, credential)
                    } else {
                        sshCredentialStore.delete(saved.hostname, sshPort)
                    }

                    appModel.serverBridge.disconnectServer(saved.id)

                    when (credential.method) {
                        SshAuthMethod.PASSWORD -> appModel.serverBridge.startRemoteOverSshConnect(
                            serverId = saved.id,
                            displayName = saved.name,
                            host = saved.hostname,
                            port = sshPort.toUShort(),
                            username = credential.username,
                            password = credential.password,
                            privateKeyPem = null,
                            passphrase = null,
                            unlockMacosKeychain = credential.unlockMacosKeychain,
                            acceptUnknownHost = true,
                            workingDir = null,
                        )
                        SshAuthMethod.KEY -> appModel.serverBridge.startRemoteOverSshConnect(
                            serverId = saved.id,
                            displayName = saved.name,
                            host = saved.hostname,
                            port = sshPort.toUShort(),
                            username = credential.username,
                            password = null,
                            privateKeyPem = credential.privateKey,
                            passphrase = credential.passphrase,
                            unlockMacosKeychain = false,
                            acceptUnknownHost = true,
                            workingDir = null,
                        )
                    }
                    appModel.refreshSnapshot()
                    sshReconnectTarget = null
                    null
                } catch (e: Exception) {
                    LLog.e("SettingsSheet", "SSH reconnect failed: ${e.message}", e)
                    e.message ?: context.getString(R.string.settings_ssh_reconnect_failed)
                }
            },
        )
    }

}

@Composable
private fun ServerSettingsRow(
    server: AppServerSnapshot,
    onRename: (() -> Unit)?,
    onEdit: (() -> Unit)?,
    onRemove: () -> Unit,
) {
    var showMenu by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
        ) {
            Text(if (server.isLocal) "📱" else "🖥", fontSize = 16.sp)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(server.displayName, color = LitterTheme.textPrimary, fontSize = 13.sp)
                Text(
                    "${server.statusLabel} · ${server.connectionModeLabel}",
                    color = server.statusColor,
                    fontSize = 11.sp,
                )
            }
            IconButton(
                onClick = { showMenu = true },
                modifier = Modifier.size(28.dp),
            ) {
                Icon(
                    Icons.Default.MoreVert,
                    contentDescription = stringResource(R.string.a11y_server_actions),
                    tint = LitterTheme.textSecondary,
                )
            }
        }

        DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
            if (onEdit != null) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.common_edit)) },
                    onClick = {
                        showMenu = false
                        onEdit()
                    },
                )
            }
            if (onRename != null) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.common_rename)) },
                    onClick = {
                        showMenu = false
                        onRename()
                    },
                )
            }
            DropdownMenuItem(
                text = { Text(stringResource(R.string.common_remove)) },
                onClick = {
                    showMenu = false
                    onRemove()
                },
            )
        }
    }
}

private enum class ServerConnectionMode(val labelRes: Int, val formHeaderRes: Int) {
    LOCAL(R.string.settings_connection_local, R.string.settings_connection_local_runtime),
    SSH(R.string.settings_connection_ssh, R.string.settings_connection_ssh_host),
    DIRECT_CODEX(R.string.settings_connection_codex, R.string.settings_connection_codex_server),
    WEBSOCKET(R.string.settings_connection_websocket, R.string.settings_connection_codex_url),
    SLINGSHOT(R.string.settings_connection_slingshot, R.string.settings_connection_slingshot),
}

private fun isSettingsSlingshotUrl(rawUrl: String): Boolean =
    runCatching { Uri.parse(rawUrl).scheme?.equals("slingshot", ignoreCase = true) == true }
        .getOrDefault(false)

private suspend fun loadSettingsSlingshotTokens(context: Context) =
    ChatGPTOAuth.requireStoredOrRefreshedTokens(
        context,
        context.getString(R.string.discovery_sign_in_slingshot),
    )

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ServerEditSheet(
    server: AppServerSnapshot,
    onDismiss: () -> Unit,
    onSave: () -> Unit,
    onTriggerSshReconnect: (SavedServer) -> Unit,
) {
    val context = LocalContext.current
    val appModel = LocalAppModel.current
    val scope = rememberCoroutineScope()

    val savedServers = remember { SavedServerStore.load(context) }
    val originalSaved = remember(savedServers, server.serverId) {
        savedServers.firstOrNull { it.id == server.serverId }
    }

    val resolvedMode = remember(originalSaved, server.isLocal) {
        when {
            server.isLocal -> ServerConnectionMode.LOCAL
            originalSaved?.websocketURL?.let(::isSettingsSlingshotUrl) == true -> ServerConnectionMode.SLINGSHOT
            originalSaved?.websocketURL != null -> ServerConnectionMode.WEBSOCKET
            originalSaved?.preferredConnectionMode == "ssh" || (originalSaved?.sshPort != null && originalSaved?.hasCodexServer == false) -> ServerConnectionMode.SSH
            else -> ServerConnectionMode.DIRECT_CODEX
        }
    }
    var displayName by remember { mutableStateOf(originalSaved?.name?.trim()?.takeIf { it.isNotEmpty() } ?: server.displayName) }
    var connectionMode by remember { mutableStateOf(resolvedMode) }
    var host by remember { mutableStateOf(originalSaved?.hostname?.trim()?.takeIf { it.isNotEmpty() } ?: server.host) }
    var codexPort by remember { mutableStateOf(originalSaved?.preferredCodexPort?.toString() ?: originalSaved?.port?.takeIf { it > 0 }?.toString() ?: "8390") }
    var websocketURL by remember { mutableStateOf(originalSaved?.websocketURL ?: "") }
    var sshPort by remember { mutableStateOf(originalSaved?.sshPort?.toString() ?: "22") }
    var wakeMAC by remember { mutableStateOf(originalSaved?.wakeMAC ?: "") }
    var validationError by remember { mutableStateOf<String?>(null) }
    var isReconnecting by remember { mutableStateOf(false) }
    var pendingSlingshotReconnect by remember { mutableStateOf<SavedServer?>(null) }

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    fun validateAndBuild(): SavedServer? {
        val name = displayName.trim()
        if (name.isEmpty()) {
            validationError = context.getString(R.string.settings_server_name_required)
            return null
        }

        if (originalSaved?.alleycatNodeId != null || originalSaved?.alleycatAgentWire == "ssh-bridge") {
            // Paired server — only name is editable
            return originalSaved.copy(name = name)
        }

        return when (connectionMode) {
            ServerConnectionMode.LOCAL -> {
                SavedServer(
                    id = server.serverId,
                    name = name,
                    hostname = "127.0.0.1",
                    port = 0,
                    codexPorts = emptyList(),
                    sshPort = null,
                    source = "local",
                    hasCodexServer = true,
                    wakeMAC = null,
                    preferredConnectionMode = null,
                    preferredCodexPort = null,
                    sshPortForwardingEnabled = null,
                    websocketURL = null,
                    rememberedByUser = true,
                    detachedTransport = false,
                )
            }
            ServerConnectionMode.SSH -> {
                val resolvedHost = host.trim()
                if (resolvedHost.isEmpty()) {
                    validationError = context.getString(R.string.settings_host_required)
                    return null
                }
                val resolvedSSHPort = sshPort.trim().toIntOrNull()
                if (resolvedSSHPort == null || resolvedSSHPort !in 1..65535) {
                    validationError = context.getString(R.string.discovery_invalid_ssh_port)
                    return null
                }
                val wakeInput = wakeMAC.trim()
                val resolvedWakeMAC = SavedServer.normalizeWakeMac(wakeInput)
                if (wakeInput.isNotEmpty() && resolvedWakeMAC == null) {
                    validationError = context.getString(R.string.discovery_invalid_wake_mac)
                    return null
                }
                SavedServer(
                    id = server.serverId,
                    name = name,
                    hostname = resolvedHost,
                    port = 0,
                    codexPorts = emptyList(),
                    sshPort = resolvedSSHPort,
                    source = "manual",
                    hasCodexServer = false,
                    wakeMAC = resolvedWakeMAC,
                    preferredConnectionMode = "ssh",
                    preferredCodexPort = null,
                    sshPortForwardingEnabled = null,
                    websocketURL = null,
                    rememberedByUser = true,
                    detachedTransport = false,
                )
            }
            ServerConnectionMode.DIRECT_CODEX -> {
                val resolvedHost = host.trim()
                if (resolvedHost.isEmpty()) {
                    validationError = context.getString(R.string.settings_host_required)
                    return null
                }
                val resolvedCodexPort = codexPort.trim().toIntOrNull()
                if (resolvedCodexPort == null || resolvedCodexPort !in 1..65535) {
                    validationError = context.getString(R.string.settings_invalid_codex_port)
                    return null
                }
                SavedServer(
                    id = server.serverId,
                    name = name,
                    hostname = resolvedHost,
                    port = resolvedCodexPort,
                    codexPorts = listOf(resolvedCodexPort),
                    sshPort = null,
                    source = "manual",
                    hasCodexServer = true,
                    wakeMAC = null,
                    preferredConnectionMode = "directCodex",
                    preferredCodexPort = resolvedCodexPort,
                    sshPortForwardingEnabled = null,
                    websocketURL = null,
                    rememberedByUser = true,
                    detachedTransport = false,
                )
            }
            ServerConnectionMode.WEBSOCKET -> {
                val rawURL = websocketURL.trim()
                if (!rawURL.startsWith("ws://", ignoreCase = true) && !rawURL.startsWith("wss://", ignoreCase = true)) {
                    validationError = context.getString(R.string.settings_invalid_websocket_url)
                    return null
                }
                val uri = runCatching { java.net.URI(rawURL) }.getOrNull()
                if (uri == null || uri.host.isNullOrEmpty()) {
                    validationError = context.getString(R.string.settings_invalid_websocket_url)
                    return null
                }
                val resolvedPort = if (uri.port != -1) uri.port else null
                SavedServer(
                    id = server.serverId,
                    name = name,
                    hostname = uri.host,
                    port = resolvedPort ?: 0,
                    codexPorts = if (resolvedPort != null) listOf(resolvedPort) else emptyList(),
                    sshPort = null,
                    source = "manual",
                    hasCodexServer = true,
                    wakeMAC = null,
                    preferredConnectionMode = "directCodex",
                    preferredCodexPort = resolvedPort,
                    sshPortForwardingEnabled = null,
                    websocketURL = rawURL,
                    rememberedByUser = true,
                    detachedTransport = false,
                )
            }
            ServerConnectionMode.SLINGSHOT -> {
                val saved = originalSaved ?: run {
                    validationError = context.getString(R.string.settings_readd_computer)
                    return null
                }
                saved.copy(
                    name = name,
                    rememberedByUser = true,
                    detachedTransport = false,
                )
            }
        }
    }

    fun persist(saved: SavedServer) {
        val existing = SavedServerStore.load(context).toMutableList()
        existing.removeAll { it.id == saved.id }
        existing.add(saved)
        SavedServerStore.save(context, existing)
        appModel.reconnectController.setMultiClankerAndQuicEnabled(true)
        appModel.reconnectController.syncSavedServers(
            existing.filter { it.rememberedByUser }.map { it.toRecord(context) }
        )
        appModel.store.renameServer(saved.id, saved.name)
    }

    suspend fun reconnect(serverId: String) {
        val servers = SavedServerStore.load(context).map { it.toRecord(context) }
        appModel.reconnectController.setMultiClankerAndQuicEnabled(true)
        appModel.reconnectController.syncSavedServers(servers)
        val result = appModel.reconnectController.reconnectServer(serverId)
        if (result.needsLocalAuthRestore) {
            appModel.restoreStoredLocalAuthState(result.serverId)
            runCatching { appModel.refreshSessions(listOf(result.serverId)) }
        }
        appModel.refreshSnapshot()
    }

    suspend fun connectSlingshotSaved(saved: SavedServer, stepUpToken: String) {
        val websocketURL = saved.websocketURL?.takeIf(::isSettingsSlingshotUrl)
            ?: throw IllegalStateException(context.getString(R.string.settings_invalid_slingshot_server))

        val tokens = loadSettingsSlingshotTokens(context)
        appModel.serverBridge.connectRemoteSlingshotUrlServer(
            saved.id,
            saved.name,
            websocketURL,
            tokens.accessToken,
            tokens.accountId,
            stepUpToken,
        )
        appModel.refreshSnapshot()
    }

    suspend fun reconnectSaved(saved: SavedServer) {
        if (saved.websocketURL?.let(::isSettingsSlingshotUrl) != true) {
            reconnect(saved.id)
            return
        }

        connectSlingshotSaved(saved, "")
    }

    val slingshotStepUpLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val saved = pendingSlingshotReconnect
        pendingSlingshotReconnect = null
        if (saved == null) {
            return@rememberLauncherForActivityResult
        }
        if (result.resultCode != android.app.Activity.RESULT_OK) {
            validationError = result.data?.getStringExtra(ChatGPTOAuthActivity.EXTRA_ERROR)
                ?: context.getString(R.string.discovery_remote_auth_cancelled)
            isReconnecting = false
            return@rememberLauncherForActivityResult
        }
        val stepUpToken = ChatGPTOAuthActivity.parseRemoteControlStepUpToken(result.data)
        if (stepUpToken == null) {
            validationError = context.getString(R.string.discovery_remote_auth_incomplete)
            isReconnecting = false
            return@rememberLauncherForActivityResult
        }

        scope.launch {
            isReconnecting = true
            try {
                connectSlingshotSaved(saved, stepUpToken)
                onSave()
            } catch (e: Exception) {
                validationError = e.message
            } finally {
                isReconnecting = false
            }
        }
    }

    fun launchSlingshotStepUp(saved: SavedServer) {
        try {
            pendingSlingshotReconnect = saved
            slingshotStepUpLauncher.launch(
                ChatGPTOAuthActivity.createIntent(
                    context,
                    ChatGPTOAuth.createRemoteControlEnrollmentAttempt(),
                ),
            )
        } catch (e: Exception) {
            pendingSlingshotReconnect = null
            validationError = e.localizedMessage
                ?: e.message
                ?: context.getString(R.string.discovery_authorize_failed)
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = LitterTheme.background,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .imePadding()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Header
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Spacer(Modifier.weight(1f))
                Text(stringResource(R.string.settings_edit_server), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.weight(1f))
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.settings_done), color = LitterTheme.accent) }
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                item {
                    SectionHeader(stringResource(R.string.settings_name))
                    OutlinedTextField(
                        value = displayName,
                        onValueChange = { displayName = it },
                        label = { Text(stringResource(R.string.settings_server_name)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                        textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                    )
                }

                item {
                    SectionHeader(stringResource(connectionMode.formHeaderRes))

                    if (originalSaved?.alleycatNodeId != null || originalSaved?.alleycatAgentWire == "ssh-bridge") {
                        Text(
                            stringResource(R.string.settings_pairing_metadata_help),
                            color = LitterTheme.textSecondary,
                            fontSize = 12.sp,
                        )
                    } else if (server.isLocal) {
                        Text(
                            stringResource(R.string.settings_local_runtime_managed),
                            color = LitterTheme.textSecondary,
                            fontSize = 12.sp,
                        )
                    } else if (connectionMode == ServerConnectionMode.SLINGSHOT) {
                        Text(
                            stringResource(R.string.settings_connected_computer_help),
                            color = LitterTheme.textSecondary,
                            fontSize = 12.sp,
                        )
                    } else {
                        // Mode selector
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
                                .padding(4.dp),
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            val modes = listOf(
                                ServerConnectionMode.SSH,
                                ServerConnectionMode.DIRECT_CODEX,
                                ServerConnectionMode.WEBSOCKET,
                            )
                            modes.forEach { mode ->
                                val selected = mode == connectionMode
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .clip(RoundedCornerShape(8.dp))
                                        .background(if (selected) LitterTheme.accent else Color.Transparent)
                                        .clickable { connectionMode = mode }
                                        .padding(vertical = 9.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        stringResource(mode.labelRes),
                                        color = if (selected) LitterTheme.onAccentStrong else LitterTheme.textSecondary,
                                        fontSize = 12.sp,
                                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                                    )
                                }
                            }
                        }

                        Spacer(Modifier.height(8.dp))

                        when (connectionMode) {
                            ServerConnectionMode.SSH -> {
                                OutlinedTextField(
                                    value = host,
                                    onValueChange = { host = it },
                                    label = { Text(stringResource(R.string.discovery_hostname_or_ip)) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                                OutlinedTextField(
                                    value = sshPort,
                                    onValueChange = { sshPort = it },
                                    label = { Text(stringResource(R.string.discovery_ssh_port)) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                                OutlinedTextField(
                                    value = wakeMAC,
                                    onValueChange = { wakeMAC = it },
                                    label = { Text(stringResource(R.string.discovery_wake_mac_optional)) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                            }
                            ServerConnectionMode.DIRECT_CODEX -> {
                                OutlinedTextField(
                                    value = host,
                                    onValueChange = { host = it },
                                    label = { Text(stringResource(R.string.discovery_hostname_or_ip)) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                                OutlinedTextField(
                                    value = codexPort,
                                    onValueChange = { codexPort = it },
                                    label = { Text(stringResource(R.string.settings_codex_port)) },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                            }
                            ServerConnectionMode.WEBSOCKET -> {
                                OutlinedTextField(
                                    value = websocketURL,
                                    onValueChange = { websocketURL = it },
                                    label = { Text("ws://host:port or wss://...") },
                                    singleLine = true,
                                    modifier = Modifier.fillMaxWidth(),
                                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                                )
                            }
                            else -> Unit
                        }

                        if (connectionMode == ServerConnectionMode.WEBSOCKET) {
                            Text(
                                stringResource(R.string.settings_manual_codex_warning),
                                color = LitterTheme.textMuted,
                                fontSize = 11.sp,
                                modifier = Modifier.padding(top = 4.dp),
                            )
                        }
                    }
                }

                item {
                    if (isReconnecting) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                            CircularProgressIndicator(color = LitterTheme.accent, strokeWidth = 2.dp)
                        }
                    } else {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(
                                onClick = {
                                    validationError = null
                                    val saved = validateAndBuild()
                                    if (saved != null) {
                                        persist(saved)
                                        onSave()
                                    }
                                },
                                colors = ButtonDefaults.buttonColors(containerColor = LitterTheme.accent),
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text(stringResource(R.string.common_save), color = LitterTheme.onAccentStrong)
                            }
                            if (server.isLocal || (originalSaved?.alleycatNodeId == null && originalSaved?.alleycatAgentWire != "ssh-bridge")) {
                                Button(
                                    onClick = {
                                        validationError = null
                                        val saved = validateAndBuild()
                                        if (saved != null) {
                                            persist(saved)
                                            // SSH mode requires interactive credentials, mirroring iOS:
                                            // hand off to the parent which will open SSHLoginDialog.
                                            if (connectionMode == ServerConnectionMode.SSH && !server.isLocal) {
                                                onTriggerSshReconnect(saved)
                                                return@Button
                                            }
                                            scope.launch {
                                                isReconnecting = true
                                                try {
                                                    reconnectSaved(saved)
                                                    onSave()
                                                } catch (e: Exception) {
                                                    isReconnecting = false
                                                    if (
                                                        connectionMode == ServerConnectionMode.SLINGSHOT &&
                                                        ChatGPTOAuth.isRemoteControlAuthorizationRequired(e)
                                                    ) {
                                                        launchSlingshotStepUp(saved)
                                                    } else {
                                                        validationError = e.message
                                                    }
                                                } finally {
                                                    if (pendingSlingshotReconnect == null) {
                                                        isReconnecting = false
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    colors = ButtonDefaults.buttonColors(containerColor = LitterTheme.accentStrong),
                                    modifier = Modifier.fillMaxWidth(),
                                ) {
                                    Text(
                                        stringResource(
                                            if (server.isLocal) {
                                                R.string.settings_save_restart
                                            } else {
                                                R.string.settings_save_reconnect
                                            },
                                        ),
                                        color = LitterTheme.background,
                                    )
                                }
                            }
                        }
                    }
                }

                item { Spacer(Modifier.height(32.dp)) }
            }
        }
    }

    validationError?.let { error ->
        AlertDialog(
            onDismissRequest = { validationError = null },
            title = { Text(stringResource(R.string.settings_invalid_server)) },
            text = { Text(error) },
            confirmButton = {
                TextButton(onClick = { validationError = null }) {
                    Text(stringResource(R.string.common_ok))
                }
            },
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Appearance Sub-Screen (matches iOS AppearanceSettingsView)
// ═══════════════════════════════════════════════════════════════════════════════

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AppearanceScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var textSizeStep by remember { mutableFloatStateOf(com.litter.android.ui.TextSizePrefs.currentStep.toFloat()) }
    var showThemePicker by remember { mutableStateOf<LitterColorThemeType?>(null) }
    var wallpaperError by remember { mutableStateOf<String?>(null) }
    val appearanceMode = LitterThemeManager.appearanceMode
    val wallpaperPicker =
        rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
            if (uri == null) {
                return@rememberLauncherForActivityResult
            }
            scope.launch {
                wallpaperError =
                    if (WallpaperManager.setCustomFromUri(uri)) {
                        null
                    } else {
                        context.getString(R.string.settings_wallpaper_save_failed)
                    }
            }
        }

    Column(
        Modifier
            .fillMaxSize()
            .imePadding()
            .padding(16.dp),
    ) {
        // Nav bar
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = LitterTheme.accent)
            }
            Spacer(Modifier.weight(1f))
            Text(stringResource(R.string.appearance_title), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.weight(1f))
            Spacer(Modifier.width(48.dp))
        }

        Spacer(Modifier.height(16.dp))

        LazyColumn(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            // Appearance mode
            item { SectionHeader("Mode") }
            item {
                AppearanceModePicker(
                    selectedMode = appearanceMode,
                    onSelect = LitterThemeManager::applyAppearanceMode,
                )
            }
            item {
                Text(
                    stringResource(R.string.settings_appearance_help),
                    color = LitterTheme.textMuted,
                    fontSize = 11.sp,
                    modifier = Modifier.padding(start = 4.dp),
                )
            }

            // Font size slider
            item { SectionHeader(stringResource(R.string.appearance_font_size)) }
            item {
                Column(
                    Modifier.fillMaxWidth()
                        .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
                        .padding(12.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Text(stringResource(R.string.appearance_font_size), color = LitterTheme.textPrimary, fontSize = 14.sp)
                        Spacer(Modifier.weight(1f))
                        val label = com.litter.android.ui.ConversationTextSize.fromStep(textSizeStep.toInt()).label
                        Text(label, color = LitterTheme.textSecondary, fontSize = 13.sp)
                    }
                    Spacer(Modifier.height(8.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("A", color = LitterTheme.textMuted, fontSize = 11.sp)
                        Slider(
                            value = textSizeStep,
                            onValueChange = {
                                textSizeStep = it
                                com.litter.android.ui.TextSizePrefs.setStep(context, it.toInt())
                            },
                            valueRange = 0f..6f, steps = 5,
                            modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
                            colors = SliderDefaults.colors(thumbColor = LitterTheme.accent, activeTrackColor = LitterTheme.accent),
                        )
                        Text("A", color = LitterTheme.textMuted, fontSize = 18.sp)
                    }
                }
            }
            item {
                Text(stringResource(R.string.appearance_font_size_helper), color = LitterTheme.textMuted, fontSize = 11.sp, modifier = Modifier.padding(start = 4.dp))
            }

            // Wallpaper picker
            item { SectionHeader(stringResource(R.string.settings_chat_wallpaper)) }
            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
                        .padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .size(width = 48.dp, height = 72.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .border(1.dp, LitterTheme.border.copy(alpha = 0.5f), RoundedCornerShape(8.dp)),
                    ) {
                        WallpaperBackdrop(modifier = Modifier.fillMaxSize())
                    }
                    Column(
                        modifier = Modifier.weight(1f),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        TextButton(
                            onClick = { wallpaperPicker.launch("image/*") },
                            contentPadding = ButtonDefaults.TextButtonContentPadding,
                        ) {
                            Text(stringResource(R.string.appearance_choose_library), color = LitterTheme.accent)
                        }
                        if (WallpaperManager.isWallpaperSet) {
                            TextButton(
                                onClick = {
                                    WallpaperManager.clear()
                                    wallpaperError = null
                                },
                                contentPadding = ButtonDefaults.TextButtonContentPadding,
                            ) {
                                Text(stringResource(R.string.appearance_remove_wallpaper), color = LitterTheme.danger)
                            }
                        }
                        if (!wallpaperError.isNullOrBlank()) {
                            Text(
                                wallpaperError!!,
                                color = LitterTheme.danger,
                                fontSize = 11.sp,
                            )
                        }
                    }
                }
            }

            // Conversation preview
            item { SectionHeader("Preview") }
            item {
                val scale = com.litter.android.ui.ConversationTextSize.fromStep(textSizeStep.toInt()).scale
                val previewFontSize = (14f * scale).sp
                val previewCodeFontSize = (13f * scale).sp
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                ) {
                    WallpaperBackdrop(modifier = Modifier.fillMaxSize())
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        // User bubble
                        Text(
                            "Hey, why is prod on fire",
                            color = LitterTheme.textPrimary,
                            fontSize = previewFontSize,
                            lineHeight = (previewFontSize.value * 1.3f).sp,
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface.copy(alpha = 0.5f), RoundedCornerShape(12.dp))
                                .padding(10.dp),
                        )
                        // Tool call card
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface, RoundedCornerShape(8.dp))
                                .padding(8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text("✓", color = LitterTheme.success, fontSize = 12.sp)
                            Spacer(Modifier.width(6.dp))
                            Text("rg 'TODO: fix later' --count", color = LitterTheme.toolCallCommand, fontFamily = BerkeleyMono, fontSize = (previewFontSize.value - 2).sp)
                            Spacer(Modifier.weight(1f))
                            Text("0.3s", color = LitterTheme.textMuted, fontSize = 10.sp)
                        }
                        // Assistant bubble
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text(
                                "Found the issue. Someone deployed this:",
                                color = LitterTheme.textBody,
                                fontSize = previewFontSize,
                                lineHeight = (previewFontSize.value * 1.3f).sp,
                            )
                            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text(
                                    "PYTHON",
                                    color = LitterTheme.textSecondary,
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Bold,
                                )
                                Box(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .background(LitterTheme.codeBackground, RoundedCornerShape(8.dp))
                                        .padding(10.dp),
                                ) {
                                    Text(
                                        "if is_friday():\n    yolo_deploy(skip_tests=True)",
                                        color = LitterTheme.textBody,
                                        fontFamily = LitterTheme.monoFont,
                                        fontSize = previewCodeFontSize,
                                        lineHeight = (previewCodeFontSize.value * 1.35f).sp,
                                    )
                                }
                            }
                            Text(
                                "I'm not mad, just disappointed.",
                                color = LitterTheme.textBody,
                                fontSize = previewFontSize,
                                lineHeight = (previewFontSize.value * 1.3f).sp,
                            )
                        }
                        // User reply
                        Text(
                            "That was you",
                            color = LitterTheme.textPrimary,
                            fontSize = previewFontSize,
                            lineHeight = (previewFontSize.value * 1.3f).sp,
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(LitterTheme.surface.copy(alpha = 0.5f), RoundedCornerShape(12.dp))
                                .padding(10.dp),
                        )
                    }
                }
            }

            // Light theme picker
            item { SectionHeader(stringResource(R.string.appearance_light_theme)) }
            item {
                val selectedLight = LitterThemeManager.lightThemes.firstOrNull {
                    it.slug == LitterThemeManager.lightTheme.slug
                } ?: LitterThemeManager.lightThemes.firstOrNull()
                ThemePickerButton(entry = selectedLight, onClick = { showThemePicker = LitterColorThemeType.LIGHT })
            }

            // Dark theme picker
            item { SectionHeader(stringResource(R.string.appearance_dark_theme)) }
            item {
                val selectedDark = LitterThemeManager.darkThemes.firstOrNull {
                    it.slug == LitterThemeManager.darkTheme.slug
                } ?: LitterThemeManager.darkThemes.firstOrNull()
                ThemePickerButton(entry = selectedDark, onClick = { showThemePicker = LitterColorThemeType.DARK })
            }
        }
    }

    // Theme picker sheet
    showThemePicker?.let { type ->
        ModalBottomSheet(
            onDismissRequest = { showThemePicker = null },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = LitterTheme.background,
        ) {
            val themes = if (type == LitterColorThemeType.DARK) LitterThemeManager.darkThemes else LitterThemeManager.lightThemes
            val selectedSlug = if (type == LitterColorThemeType.DARK) LitterThemeManager.darkTheme.slug else LitterThemeManager.lightTheme.slug
            ThemePickerContent(
                title = if (type == LitterColorThemeType.DARK) {
                    stringResource(R.string.appearance_dark_theme)
                } else {
                    stringResource(R.string.appearance_light_theme)
                },
                themes = themes,
                selectedSlug = selectedSlug,
                onSelect = { slug ->
                    if (type == LitterColorThemeType.DARK) {
                        LitterThemeManager.selectDarkTheme(slug)
                    } else {
                        LitterThemeManager.selectLightTheme(slug)
                    }
                    showThemePicker = null
                },
                onDismiss = { showThemePicker = null },
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Theme Picker Sheet (matches iOS ThemePickerSheet)
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun ThemePickerContent(
    title: String,
    themes: List<LitterThemeIndexEntry>,
    selectedSlug: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var searchQuery by remember { mutableStateOf("") }
    val filtered = remember(themes, searchQuery) {
        if (searchQuery.isBlank()) themes
        else themes.filter { it.name.contains(searchQuery, ignoreCase = true) || it.slug.contains(searchQuery, ignoreCase = true) }
    }

    Column(
        Modifier
            .fillMaxWidth()
            .imePadding()
            .padding(16.dp),
    ) {
        // Title + Done
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Spacer(Modifier.weight(1f))
            Text(title, color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.weight(1f))
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.settings_done), color = LitterTheme.accent) }
        }

        Spacer(Modifier.height(8.dp))

        // Search
        Row(
            Modifier.fillMaxWidth()
                .background(LitterTheme.surface.copy(alpha = 0.55f), RoundedCornerShape(10.dp))
                .border(1.dp, LitterTheme.border.copy(alpha = 0.85f), RoundedCornerShape(10.dp))
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Default.Search, null, tint = LitterTheme.textMuted, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(8.dp))
            BasicTextField(
                value = searchQuery, onValueChange = { searchQuery = it },
                textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 14.sp),
                cursorBrush = SolidColor(LitterTheme.accent),
                modifier = Modifier.fillMaxWidth(),
                decorationBox = { inner ->
                    if (searchQuery.isEmpty()) Text(stringResource(R.string.appearance_search_themes), color = LitterTheme.textMuted, fontSize = 14.sp)
                    inner()
                },
            )
        }

        Spacer(Modifier.height(12.dp))

        // Theme list
        if (filtered.isEmpty()) {
            Column(Modifier.fillMaxWidth().padding(top = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(Icons.Default.Search, null, tint = LitterTheme.textMuted, modifier = Modifier.size(24.dp))
                Spacer(Modifier.height(8.dp))
                Text(stringResource(R.string.appearance_no_matching_themes), color = LitterTheme.textPrimary, fontSize = 14.sp)
            }
        } else {
            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(filtered, key = { it.slug }) { entry ->
                    val isSelected = entry.slug == selectedSlug
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth()
                            .background(LitterTheme.surface.copy(alpha = 0.72f), RoundedCornerShape(12.dp))
                            .border(
                                1.dp,
                                if (isSelected) LitterTheme.accent.copy(alpha = 0.6f) else LitterTheme.border.copy(alpha = 0.85f),
                                RoundedCornerShape(12.dp),
                            )
                            .clickable { onSelect(entry.slug) }
                            .padding(horizontal = 12.dp, vertical = 11.dp),
                    ) {
                        ThemePreviewBadge(entry)
                        Spacer(Modifier.width(10.dp))
                        Text(entry.name, color = LitterTheme.textPrimary, fontSize = 14.sp, modifier = Modifier.weight(1f))
                        if (isSelected) {
                            Icon(Icons.Default.Check, null, tint = LitterTheme.accent, modifier = Modifier.size(16.dp))
                        }
                    }
                }
            }
        }
    }
}

/** "Aa" badge with background/foreground/accent dot — matches iOS ThemePreviewBadge */
@Composable
private fun ThemePreviewBadge(entry: LitterThemeIndexEntry) {
    val bg = try { Color(android.graphics.Color.parseColor(entry.backgroundHex)) } catch (_: Exception) { LitterTheme.surface }
    val fg = try { Color(android.graphics.Color.parseColor(entry.foregroundHex)) } catch (_: Exception) { LitterTheme.textPrimary }
    val accent = try { Color(android.graphics.Color.parseColor(entry.accentHex)) } catch (_: Exception) { LitterTheme.accent }

    Box {
        Box(
            Modifier.size(width = 28.dp, height = 22.dp)
                .background(bg, RoundedCornerShape(5.dp))
                .border(0.5.dp, Color.Gray.copy(alpha = 0.3f), RoundedCornerShape(5.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Text("Aa", color = fg, fontSize = 11.sp, fontWeight = FontWeight.Bold, fontFamily = BerkeleyMono)
        }
        Spacer(
            Modifier.size(6.dp).clip(CircleShape).background(accent)
                .align(Alignment.BottomEnd),
        )
    }
}

@Composable
private fun ThemePickerButton(entry: LitterThemeIndexEntry?, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth()
            .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(12.dp),
    ) {
        if (entry != null) {
            ThemePreviewBadge(entry)
            Spacer(Modifier.width(10.dp))
            Text(entry.name, color = LitterTheme.textPrimary, fontSize = 14.sp, modifier = Modifier.weight(1f))
        } else {
            Text(stringResource(R.string.appearance_no_themes), color = LitterTheme.textMuted, fontSize = 14.sp, modifier = Modifier.weight(1f))
        }
        Text("⇅", color = LitterTheme.textMuted, fontSize = 12.sp)
    }
}

@Composable
private fun AppearanceModePicker(
    selectedMode: LitterAppearanceMode,
    onSelect: (LitterAppearanceMode) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        LitterAppearanceMode.entries.forEach { mode ->
            val isSelected = mode == selectedMode
            Box(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (isSelected) LitterTheme.accent else Color.Transparent)
                    .clickable { onSelect(mode) }
                    .padding(vertical = 9.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = mode.displayName,
                    color = if (isSelected) LitterTheme.onAccentStrong else LitterTheme.textSecondary,
                    fontSize = 12.sp,
                    fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
                )
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pets Sub-Screen
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun PetsScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val appModel = LocalAppModel.current
    val snapshot by appModel.snapshot.collectAsState()
    val scope = rememberCoroutineScope()
    val connectedServers = remember(snapshot) {
        snapshot?.servers.orEmpty().filter { it.isConnected }
    }
    var selectedServerId by remember(connectedServers) {
        mutableStateOf(
            PetOverlayController.selectedPet?.serverId?.takeIf { id ->
                connectedServers.any { it.serverId == id }
            }
                ?: snapshot?.activeThread?.serverId?.takeIf { id ->
                    connectedServers.any { it.serverId == id }
                }
                ?: connectedServers.firstOrNull()?.serverId
                ?: "",
        )
    }
    var pets by remember(selectedServerId) { mutableStateOf<List<AppPetSummary>>(emptyList()) }
    var loading by remember(selectedServerId) { mutableStateOf(false) }
    var error by remember(selectedServerId) { mutableStateOf<String?>(null) }
    val overlayPermissionGranted = PetOverlayController.canDrawOverlays(context)

    fun refresh() {
        if (selectedServerId.isBlank()) return
        scope.launch {
            loading = true
            error = null
            runCatching { appModel.client.listPets(selectedServerId) }
                .onSuccess { pets = it }
                .onFailure {
                    pets = emptyList()
                    error = it.message ?: context.getString(R.string.pet_load_unavailable)
                }
            loading = false
        }
    }

    LaunchedEffect(selectedServerId) {
        refresh()
    }

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .imePadding()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = LitterTheme.accent)
                }
                Spacer(Modifier.weight(1f))
                Text(stringResource(R.string.pet_title), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.weight(1f))
                IconButton(onClick = { refresh() }, enabled = selectedServerId.isNotBlank() && !loading) {
                    Icon(Icons.Default.Refresh, "Refresh", tint = LitterTheme.accent)
                }
            }
        }

        item { SectionHeader("Wake") }
        item {
            SettingsRow(
                label = stringResource(R.string.pet_show),
                subtitle = PetOverlayController.selectedPet?.displayName ?: stringResource(R.string.pet_none_selected),
                icon = { Icon(Icons.Default.Pets, null, tint = LitterTheme.accent, modifier = Modifier.size(18.dp)) },
                trailing = {
                    Switch(
                        checked = PetOverlayController.visible,
                        onCheckedChange = { PetOverlayController.setVisible(context, it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accent),
                    )
                },
            )
        }
        item {
            SettingsRow(
                label = stringResource(R.string.pet_show_over_apps),
                subtitle = if (overlayPermissionGranted) {
                    stringResource(R.string.settings_overlay_granted)
                } else {
                    stringResource(R.string.settings_overlay_needed)
                },
                icon = { Icon(Icons.Default.Widgets, null, tint = LitterTheme.accent, modifier = Modifier.size(18.dp)) },
                trailing = {
                    Switch(
                        checked = PetOverlayController.overlayEnabled,
                        onCheckedChange = { enabled ->
                            PetOverlayController.setOverlayEnabled(context, enabled)
                            if (enabled && !overlayPermissionGranted) {
                                PetOverlayController.requestOverlayPermission(context)
                            }
                        },
                        colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accent),
                    )
                },
                onClick = if (!overlayPermissionGranted) {
                    { PetOverlayController.requestOverlayPermission(context) }
                } else {
                    null
                },
            )
        }

        item { SectionHeader("Server") }
        if (connectedServers.isEmpty()) {
            item { SettingsRow(label = stringResource(R.string.settings_connect_server_first)) }
        } else {
            items(connectedServers, key = { it.serverId }) { server ->
                SettingsRow(
                    label = server.displayName,
                    subtitle = server.connectionModeLabel,
                    trailing = {
                        if (server.serverId == selectedServerId) {
                            Icon(Icons.Default.Check, null, tint = LitterTheme.accentStrong, modifier = Modifier.size(18.dp))
                        }
                    },
                    onClick = { selectedServerId = server.serverId },
                )
            }
        }

        item { SectionHeader("Pets") }
        when {
            selectedServerId.isBlank() -> {
                item { SettingsRow(label = stringResource(R.string.pet_no_server_selected)) }
            }
            loading -> {
                item {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
                            .padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(18.dp), color = LitterTheme.accent, strokeWidth = 2.dp)
                        Spacer(Modifier.width(10.dp))
                        Text(stringResource(R.string.pet_loading), color = LitterTheme.textSecondary, fontSize = 13.sp)
                    }
                }
            }
            error != null -> {
                item { SettingsRow(label = stringResource(R.string.pet_load_unavailable), subtitle = error) }
            }
            pets.isEmpty() -> {
                item {
                    SettingsRow(
                        label = stringResource(R.string.pet_none_found),
                        subtitle = stringResource(R.string.pet_none_found_detail),
                    )
                }
            }
            else -> {
                items(pets, key = { it.id }) { pet ->
                    val selected = PetOverlayController.selectedPet?.serverId == selectedServerId &&
                        PetOverlayController.selectedPet?.id == pet.id
                    SettingsRow(
                        label = pet.displayName,
                        subtitle = pet.validationError ?: pet.description ?: pet.sourcePath,
                        trailing = {
                            if (PetOverlayController.isLoading && selected) {
                                CircularProgressIndicator(modifier = Modifier.size(18.dp), color = LitterTheme.accent, strokeWidth = 2.dp)
                            } else if (selected) {
                                Icon(Icons.Default.Check, null, tint = LitterTheme.accentStrong, modifier = Modifier.size(18.dp))
                            }
                        },
                        onClick = if (pet.hasValidSpritesheet) {
                            {
                                scope.launch {
                                    PetOverlayController.selectPet(context, appModel, selectedServerId, pet)
                                }
                            }
                        } else {
                            null
                        },
                    )
                }
            }
        }

        PetOverlayController.errorMessage?.let { message ->
            item { SettingsRow(label = stringResource(R.string.pet_load_failed), subtitle = message) }
        }

        item { Spacer(Modifier.height(32.dp)) }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Experimental Sub-Screen (matches iOS ExperimentalFeaturesView)
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun ExperimentalScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val features = remember { LitterFeature.entries }

    Column(
        Modifier
            .fillMaxSize()
            .imePadding()
            .padding(16.dp),
    ) {
        // Nav bar
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = LitterTheme.accent)
            }
            Spacer(Modifier.weight(1f))
            Text(stringResource(R.string.experimental_title), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.weight(1f))
            Spacer(Modifier.width(48.dp))
        }

        Spacer(Modifier.height(16.dp))

        SectionHeader("Features")
        Column(
            Modifier.fillMaxWidth().background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)),
        ) {
            features.forEachIndexed { idx, feature ->
                val enabled = ExperimentalFeatures.isEnabled(feature)
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp),
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(feature.displayName, color = LitterTheme.textPrimary, fontSize = 14.sp)
                        Text(feature.description, color = LitterTheme.textSecondary, fontSize = 11.sp)
                    }
                    Switch(
                        checked = enabled,
                        onCheckedChange = { ExperimentalFeatures.setEnabled(context, feature, it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accentStrong),
                    )
                }
                if (idx < features.lastIndex) HorizontalDivider(color = LitterTheme.divider)
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.experimental_warning), color = LitterTheme.textMuted, fontSize = 11.sp, modifier = Modifier.padding(start = 4.dp))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Debug Sub-Screen
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun DebugScreen(onBack: () -> Unit) {
    val context = LocalContext.current

    Column(
        Modifier
            .fillMaxSize()
            .imePadding()
            .padding(16.dp),
    ) {
        // Nav bar
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = LitterTheme.accent)
            }
            Spacer(Modifier.weight(1f))
            Text(stringResource(R.string.debug_title), color = LitterTheme.textPrimary, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.weight(1f))
            Spacer(Modifier.width(48.dp))
        }

        Spacer(Modifier.height(16.dp))

        SectionHeader("Rendering")
        Column(
            Modifier.fillMaxWidth().background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp),
            ) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.debug_disable_markdown), color = LitterTheme.textPrimary, fontSize = 14.sp)
                    Text(stringResource(R.string.debug_disable_markdown_detail), color = LitterTheme.textSecondary, fontSize = 11.sp)
                }
                Switch(
                    checked = DebugSettings.disableMarkdown,
                    onCheckedChange = { DebugSettings.setDisableMarkdown(context, it) },
                    colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accentStrong),
                )
            }
            HorizontalDivider(color = LitterTheme.divider)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp),
            ) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.debug_show_turn_metrics), color = LitterTheme.textPrimary, fontSize = 14.sp)
                    Text(stringResource(R.string.debug_show_turn_metrics_detail), color = LitterTheme.textSecondary, fontSize = 11.sp)
                }
                Switch(
                    checked = DebugSettings.showTurnMetrics,
                    onCheckedChange = { DebugSettings.setShowTurnMetrics(context, it) },
                    colors = SwitchDefaults.colors(checkedTrackColor = LitterTheme.accentStrong),
                )
            }
        }

        // ── Recording ──
        Spacer(Modifier.height(12.dp))
        SectionHeader(stringResource(R.string.settings_recording_section))

        val appModel = LocalAppModel.current
        val scope = rememberCoroutineScope()
        var isRecording by remember { mutableStateOf(MessageRecorder.isRecording(appModel.store)) }
        var recordings by remember { mutableStateOf(MessageRecorder.listRecordings(context)) }

        Column(
            Modifier.fillMaxWidth().background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)).padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        stringResource(
                            if (isRecording) {
                                R.string.settings_recording
                            } else {
                                R.string.settings_message_recording
                            },
                        ),
                        color = if (isRecording) LitterTheme.danger else LitterTheme.textPrimary,
                        fontSize = 14.sp,
                    )
                    Text(stringResource(R.string.debug_record_server_messages), color = LitterTheme.textSecondary, fontSize = 11.sp)
                }
                TextButton(onClick = {
                    if (isRecording) {
                        MessageRecorder.stopRecording(context, appModel.store)
                        isRecording = false
                        recordings = MessageRecorder.listRecordings(context)
                    } else {
                        MessageRecorder.startRecording(appModel.store)
                        isRecording = true
                    }
                }) {
                    Text(
                        stringResource(
                            if (isRecording) R.string.common_stop else R.string.common_start,
                        ),
                        color = if (isRecording) LitterTheme.danger else LitterTheme.accent,
                    )
                }
            }

            if (recordings.isNotEmpty()) {
                HorizontalDivider(color = LitterTheme.divider)
                Text(stringResource(R.string.debug_saved_recordings), color = LitterTheme.textSecondary, fontSize = 11.sp)
                recordings.forEach { file ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    ) {
                        Text(
                            file.name,
                            color = LitterTheme.textPrimary,
                            fontSize = 12.sp,
                            modifier = Modifier.weight(1f),
                        )
                        val sizeKb = file.length() / 1024
                        Text("${sizeKb}KB", color = LitterTheme.textMuted, fontSize = 10.sp)
                        Spacer(Modifier.width(8.dp))
                        TextButton(onClick = {
                            MessageRecorder.deleteRecording(file)
                            recordings = MessageRecorder.listRecordings(context)
                        }) {
                            Text(stringResource(R.string.common_delete), color = LitterTheme.danger, fontSize = 11.sp)
                        }
                    }
                }
            }
        }

        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.debug_warning), color = LitterTheme.textMuted, fontSize = 11.sp, modifier = Modifier.padding(start = 4.dp))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Account Section (inline in top-level, matches iOS SettingsConnectionAccountSection)
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun AccountSection(server: uniffi.codex_mobile_client.AppServerSnapshot) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val apiKeyStore = remember(context) { OpenAIApiKeyStore(context.applicationContext) }
    var apiKey by remember { mutableStateOf("") }
    var openAIBaseUrl by remember { mutableStateOf("") }
    var isAuthWorking by remember { mutableStateOf(false) }
    var authError by remember { mutableStateOf<String?>(null) }
    var hasStoredApiKey by remember { mutableStateOf(apiKeyStore.hasStoredKey()) }
    var hasStoredBaseUrl by remember { mutableStateOf(apiKeyStore.hasStoredBaseUrl()) }
    val authLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        isAuthWorking = false
        LLog.d("ChatGPTOAuth", "settings auth result", fields = mapOf("resultCode" to result.resultCode))
        if (result.resultCode == android.app.Activity.RESULT_OK) {
            val tokens = ChatGPTOAuthActivity.parseResult(result.data)
            if (tokens == null) {
                authError = context.getString(R.string.account_login_incomplete)
                LLog.w("ChatGPTOAuth", "settings auth result missing tokens")
                return@rememberLauncherForActivityResult
            }
            scope.launch {
                isAuthWorking = true
                try {
                    LLog.d("ChatGPTOAuth", "settings loginAccount starting")
                    appModel.client.loginAccount(
                        server.serverId,
                        AppLoginAccountRequest.ChatgptAuthTokens(
                            accessToken = tokens.accessToken,
                            chatgptAccountId = tokens.accountId,
                            chatgptPlanType = tokens.planType,
                        ),
                    )
                    appModel.refreshSnapshot()
                    authError = null
                    LLog.i("ChatGPTOAuth", "settings loginAccount succeeded")
                } catch (e: Exception) {
                    authError = e.localizedMessage ?: e.message
                    LLog.e("ChatGPTOAuth", "settings loginAccount failed", e)
                }
                isAuthWorking = false
            }
        } else {
            authError = result.data?.getStringExtra(ChatGPTOAuthActivity.EXTRA_ERROR)
            authError?.let { LLog.w("ChatGPTOAuth", "settings auth canceled", fields = mapOf("error" to it)) }
        }
    }

    val authColor = when (server.account) {
        is Account.Chatgpt -> LitterTheme.accent
        is Account.ApiKey -> Color(0xFF00AAFF)

        else -> LitterTheme.textMuted
    }
    val authTitle = when (val acct = server.account) {
        is Account.Chatgpt -> acct.email.ifEmpty { "ChatGPT" }
        is Account.ApiKey -> stringResource(R.string.account_api_key)

        else -> stringResource(R.string.settings_not_logged_in)
    }
    val authSubtitle = when (server.account) {
        is Account.Chatgpt -> stringResource(R.string.settings_chatgpt_account)
        is Account.ApiKey -> stringResource(R.string.settings_openai_api_key)

        else -> null
    }
    val allowsLocalEnvApiKey = server.isLocal
    val isChatGPTAccount = server.account is Account.Chatgpt

    androidx.compose.runtime.LaunchedEffect(server.serverId, server.account) {
        hasStoredApiKey = apiKeyStore.hasStoredKey()
        hasStoredBaseUrl = apiKeyStore.hasStoredBaseUrl()
    }

    Column(
        Modifier.fillMaxWidth().background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp)).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Status row
        Row(verticalAlignment = Alignment.CenterVertically) {
            Spacer(Modifier.size(10.dp).clip(CircleShape).background(authColor))
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(authTitle, color = LitterTheme.textPrimary, fontSize = 14.sp)
                authSubtitle?.let { Text(it, color = LitterTheme.textSecondary, fontSize = 11.sp) }
            }
            if (server.isLocal && server.account != null) {
                TextButton(onClick = {
                    scope.launch {
                        try {
                            ChatGPTOAuthTokenStore(context).clear()
                            apiKeyStore.clear()
                            appModel.client.logoutAccount(server.serverId)
                            appModel.restartLocalServer()
                        } catch (_: Exception) {}
                    }
                }) { Text(stringResource(R.string.account_logout), color = LitterTheme.danger, fontSize = 12.sp) }
            }
        }

        if (server.isLocal && hasStoredApiKey) {
            Text(
                stringResource(R.string.account_local_key_saved),
                color = LitterTheme.accent,
                fontSize = 11.sp,
            )
        }

        if (server.isLocal && hasStoredBaseUrl) {
            Text(
                stringResource(R.string.account_base_url_saved),
                color = LitterTheme.accent,
                fontSize = 11.sp,
            )
        }

        // Login button
        if (server.isLocal && !isChatGPTAccount) {
            Button(
                onClick = {
                    try {
                        authError = null
                        isAuthWorking = true
                        authLauncher.launch(
                            ChatGPTOAuthActivity.createIntent(
                                context,
                                ChatGPTOAuth.createLoginAttempt(),
                            ),
                        )
                    } catch (e: Exception) {
                        isAuthWorking = false
                        authError = e.localizedMessage ?: e.message
                    }
                },
                enabled = !isAuthWorking,
                colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
            ) {
                if (isAuthWorking) { CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp, color = LitterTheme.textPrimary); Spacer(Modifier.width(6.dp)) }
                Text(stringResource(R.string.account_login_chatgpt), color = LitterTheme.accent, fontSize = 14.sp)
            }
        }

        if (allowsLocalEnvApiKey) {
            if (hasStoredApiKey) {
                Text(
                    stringResource(R.string.account_key_saved_environment),
                    color = LitterTheme.textSecondary,
                    fontSize = 11.sp,
                )
            } else if (isChatGPTAccount) {
                Text(
                    stringResource(R.string.account_save_key_environment),
                    color = LitterTheme.textSecondary,
                    fontSize = 11.sp,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                BasicTextField(
                    value = apiKey, onValueChange = { apiKey = it },
                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 13.sp),
                    cursorBrush = SolidColor(LitterTheme.accent),
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.weight(1f).background(LitterTheme.codeBackground, RoundedCornerShape(6.dp)).padding(8.dp),
                    decorationBox = { inner -> if (apiKey.isEmpty()) Text("sk-...", color = LitterTheme.textMuted, fontSize = 13.sp); inner() },
                )
                Spacer(Modifier.width(8.dp))
                TextButton(
                    onClick = {
                        val key = apiKey.trim(); if (key.isEmpty()) return@TextButton
                        scope.launch {
                            isAuthWorking = true
                            try {
                                apiKeyStore.save(key)
                                if (server.account is Account.ApiKey) {
                                    appModel.client.logoutAccount(server.serverId)
                                }
                                appModel.restartLocalServer()
                                hasStoredApiKey = apiKeyStore.hasStoredKey()
                                if (hasStoredApiKey) {
                                    apiKey = ""
                                } else {
                                    authError = context.getString(R.string.account_key_not_persisted)
                                    return@launch
                                }
                                authError = null
                            } catch (e: Exception) {
                                authError = e.message
                            }
                            isAuthWorking = false
                        }
                    },
                    enabled = apiKey.trim().isNotEmpty() && !isAuthWorking,
                ) {
                    Text(
                        stringResource(
                            if (hasStoredApiKey) {
                                R.string.account_update_api_key
                            } else {
                                R.string.account_save_api_key
                            },
                        ),
                        color = LitterTheme.accent,
                        fontSize = 12.sp,
                    )
                }
            }

            Text(
                if (hasStoredBaseUrl) {
                    stringResource(R.string.account_custom_endpoint_saved)
                } else {
                    stringResource(R.string.account_optional_endpoint)
                },
                color = LitterTheme.textSecondary,
                fontSize = 11.sp,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                BasicTextField(
                    value = openAIBaseUrl,
                    onValueChange = { openAIBaseUrl = it },
                    textStyle = TextStyle(color = LitterTheme.textPrimary, fontSize = 13.sp),
                    cursorBrush = SolidColor(LitterTheme.accent),
                    modifier = Modifier.weight(1f).background(LitterTheme.codeBackground, RoundedCornerShape(6.dp)).padding(8.dp),
                    decorationBox = { inner -> if (openAIBaseUrl.isEmpty()) Text("http://host:port/v1", color = LitterTheme.textMuted, fontSize = 13.sp); inner() },
                )
                Spacer(Modifier.width(8.dp))
                TextButton(
                    onClick = {
                        val normalized = normalizeOpenAIBaseUrl(openAIBaseUrl)
                        if (normalized == null) {
                            authError = context.getString(R.string.account_invalid_base_url)
                        } else {
                            scope.launch {
                                isAuthWorking = true
                                try {
                                    apiKeyStore.saveBaseUrl(normalized)
                                    appModel.restartLocalServer()
                                    hasStoredBaseUrl = apiKeyStore.hasStoredBaseUrl()
                                    if (hasStoredBaseUrl) {
                                        openAIBaseUrl = ""
                                        authError = null
                                    } else {
                                        authError = context.getString(R.string.account_base_url_not_persisted)
                                    }
                                } catch (e: Exception) {
                                    authError = e.message
                                } finally {
                                    isAuthWorking = false
                                }
                            }
                        }
                    },
                    enabled = openAIBaseUrl.trim().isNotEmpty() && !isAuthWorking,
                ) {
                    Text(
                        stringResource(
                            if (hasStoredBaseUrl) R.string.common_update else R.string.common_save,
                        ),
                        color = LitterTheme.accent,
                        fontSize = 12.sp,
                    )
                }
            }
            if (hasStoredBaseUrl) {
                TextButton(
                    onClick = {
                        scope.launch {
                            isAuthWorking = true
                            try {
                                apiKeyStore.clearBaseUrl()
                                appModel.restartLocalServer()
                                hasStoredBaseUrl = apiKeyStore.hasStoredBaseUrl()
                                openAIBaseUrl = ""
                                authError = null
                            } catch (e: Exception) {
                                authError = e.message
                            } finally {
                                isAuthWorking = false
                            }
                        }
                    },
                    enabled = !isAuthWorking,
                ) {
                    Text(stringResource(R.string.account_clear_base_url), color = LitterTheme.danger, fontSize = 12.sp)
                }
            }
        } else {
            Text(
                stringResource(R.string.account_remote_login_help),
                color = LitterTheme.textSecondary,
                fontSize = 11.sp,
            )
        }

        authError?.let { Text(it, color = LitterTheme.danger, fontSize = 11.sp) }
    }
}

private fun normalizeOpenAIBaseUrl(rawValue: String): String? {
    val trimmed = rawValue.trim().trimEnd('/')
    if (trimmed.isEmpty()) return null
    val uri = runCatching { java.net.URI(trimmed) }.getOrNull() ?: return null
    val scheme = uri.scheme?.lowercase()
    if (scheme != "http" && scheme != "https") return null
    if (uri.host.isNullOrBlank()) return null
    return trimmed
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared Components
// ═══════════════════════════════════════════════════════════════════════════════

@Composable
private fun SectionHeader(text: String) {
    Spacer(Modifier.height(8.dp))
    Text(text.uppercase(), color = LitterTheme.textSecondary, fontSize = 11.sp, fontWeight = FontWeight.Medium, modifier = Modifier.padding(start = 4.dp, bottom = 4.dp))
}

@Composable
private fun SettingsRow(
    label: String, subtitle: String? = null,
    icon: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth()
            .background(LitterTheme.surface.copy(alpha = 0.6f), RoundedCornerShape(10.dp))
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(12.dp),
    ) {
        icon?.invoke()
        if (icon != null) Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(label, color = LitterTheme.textPrimary, fontSize = 14.sp)
            subtitle?.let { Text(it, color = LitterTheme.textSecondary, fontSize = 11.sp) }
        }
        trailing?.invoke()
    }
}

@Composable
private fun NavRow(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    SettingsRow(
        icon = { Icon(icon, null, tint = LitterTheme.accent, modifier = Modifier.size(20.dp)) },
        label = label,
        trailing = { Icon(Icons.Default.ChevronRight, null, tint = LitterTheme.textMuted, modifier = Modifier.size(16.dp)) },
        onClick = onClick,
    )
}

@Composable
private fun FontRow(name: String, fontFamily: FontFamily, isSelected: Boolean, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(name, color = LitterTheme.textPrimary, fontSize = 14.sp)
            Text(stringResource(R.string.appearance_font_preview), color = LitterTheme.textSecondary, fontSize = 13.sp, fontFamily = fontFamily)
        }
        if (isSelected) Icon(Icons.Default.Check, null, tint = LitterTheme.accent, modifier = Modifier.size(18.dp))
    }
}
