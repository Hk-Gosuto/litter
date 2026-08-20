package com.litter.android.ui.discovery

import android.content.Context
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.DesktopWindows
import androidx.compose.material.icons.outlined.DeveloperBoard
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Lan
import androidx.compose.material.icons.outlined.Laptop
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Terminal
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sigkitten.litter.android.R
import com.litter.android.state.SavedServer
import com.litter.android.state.SavedServerStore
import com.litter.android.state.SavedSshCredential
import com.litter.android.state.ChatGPTOAuth
import com.litter.android.state.SshAuthMethod
import com.litter.android.state.SshCredentialStore
import com.litter.android.state.connectionProgressDetail
import com.litter.android.state.isConnected
import com.litter.android.state.statusColor
import com.litter.android.state.statusLabel
import com.litter.android.auth.ChatGPTOAuthActivity
import com.litter.android.ui.LitterTheme
import com.litter.android.ui.LocalAppModel
import com.litter.android.ui.common.AgentIconView
import com.litter.android.ui.common.BetaBadge
import com.litter.android.ui.common.isBeta
import com.litter.android.util.LLog
import java.net.InetSocketAddress
import java.net.Socket
import java.net.URI
import java.io.File
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.codex_mobile_client.AgentAvailabilityStatus
import com.litter.android.ui.common.AgentRuntimeKind
import com.litter.android.ui.common.metadata
import com.litter.android.ui.common.runtimeLabel
import com.litter.android.ui.common.runtimeSortIndex
import uniffi.codex_mobile_client.AppSshSessionResult
import uniffi.codex_mobile_client.AppServerHealth
import uniffi.codex_mobile_client.AppServerSnapshot
import uniffi.codex_mobile_client.RemoteAgentAvailability
import uniffi.codex_mobile_client.AppSlingshotEnvironment
import uniffi.codex_mobile_client.SshBridgeTransport

private data class SshBridgeAgentContext(
    val server: SavedServer,
    val sessionId: String,
    val host: String,
    val availability: List<RemoteAgentAvailability>,
    val credential: SavedSshCredential,
)

private const val SLINGSHOT_BASE_URL = "https://chatgpt.com/backend-api"

/**
 * Server connection screen.
 */
@Composable
fun DiscoveryScreen(
    onDismiss: () -> Unit,
) {
    val logTag = "DiscoveryScreen"
    val appModel = LocalAppModel.current
    val snapshot by appModel.snapshot.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val sshCredentialStore = remember(context) { SshCredentialStore(context.applicationContext) }

    var showManualEntry by remember { mutableStateOf(false) }
    var showAlleycatSheet by remember { mutableStateOf(false) }
    var alleycatPairingMode by remember { mutableStateOf(AlleycatPairingMode.Kittylitter) }
    var showSlingshotComputers by remember { mutableStateOf(false) }
    var slingshotEnvironments by remember { mutableStateOf<List<AppSlingshotEnvironment>>(emptyList()) }
    var slingshotIsLoading by remember { mutableStateOf(false) }
    var slingshotError by remember { mutableStateOf<String?>(null) }
    var pendingManualSshServer by remember { mutableStateOf<SavedServer?>(null) }
    var sshServer by remember { mutableStateOf<SavedServer?>(null) }
    var sshAgentContext by remember { mutableStateOf<SshBridgeAgentContext?>(null) }
    var pendingAutoNavigateServerId by remember { mutableStateOf<String?>(null) }
    var pendingSlingshotEnvironment by remember { mutableStateOf<AppSlingshotEnvironment?>(null) }
    var authorizedSlingshotConnect by remember { mutableStateOf<Pair<AppSlingshotEnvironment, String>?>(null) }
    var wakingServerId by remember { mutableStateOf<String?>(null) }
    var connectError by remember { mutableStateOf<String?>(null) }
    val slingshotStepUpLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val environment = pendingSlingshotEnvironment
        pendingSlingshotEnvironment = null
        if (environment == null) {
            return@rememberLauncherForActivityResult
        }
        if (result.resultCode != android.app.Activity.RESULT_OK) {
            connectError = result.data?.getStringExtra(ChatGPTOAuthActivity.EXTRA_ERROR)
                ?: context.getString(R.string.discovery_remote_auth_cancelled)
            return@rememberLauncherForActivityResult
        }
        val stepUpToken = ChatGPTOAuthActivity.parseRemoteControlStepUpToken(result.data)
        if (stepUpToken == null) {
            connectError = context.getString(R.string.discovery_remote_auth_incomplete)
            return@rememberLauncherForActivityResult
        }
        authorizedSlingshotConnect = environment to stepUpToken
    }

    LaunchedEffect(showManualEntry, pendingManualSshServer) {
        if (!showManualEntry && pendingManualSshServer != null) {
            sshServer = pendingManualSshServer
            pendingManualSshServer = null
        }
    }

    LaunchedEffect(snapshot, pendingAutoNavigateServerId) {
        val pendingServerId = pendingAutoNavigateServerId ?: return@LaunchedEffect
        val serverSnapshot = snapshot?.servers?.firstOrNull { it.serverId == pendingServerId } ?: return@LaunchedEffect
        if (serverSnapshot.isConnected) {
            pendingAutoNavigateServerId = null
            onDismiss()
        } else if (serverSnapshot.health == AppServerHealth.DISCONNECTED) {
            serverSnapshot.connectionProgress?.terminalMessage?.let { message ->
                pendingAutoNavigateServerId = null
                connectError = message
            }
        }
    }

    suspend fun loadSlingshotEnvironments() {
        if (slingshotIsLoading) {
            return
        }
        slingshotIsLoading = true
        slingshotError = null
        try {
            val tokens = loadSlingshotTokens(context)
            slingshotEnvironments = appModel.serverBridge
                .listSlingshotEnvironments(
                    baseUrl = SLINGSHOT_BASE_URL,
                    accessToken = tokens.accessToken,
                    accountId = tokens.accountId,
                )
                .sortedWith(
                    compareByDescending<AppSlingshotEnvironment> { it.online }
                        .thenBy { it.busy }
                        .thenBy { it.displayName.lowercase() },
                )
        } catch (e: Exception) {
            LLog.e(logTag, "slingshot environment load failed", e)
            slingshotError = e.message ?: context.getString(R.string.discovery_load_computers_failed)
        } finally {
            slingshotIsLoading = false
        }
    }

    suspend fun connectSlingshotEnvironmentOrThrow(environment: AppSlingshotEnvironment, stepUpToken: String) {
        if (!environment.online) {
            throw IllegalStateException("${environment.displayName} is offline.")
        }
        val server = slingshotSavedServer(environment)
        val tokens = loadSlingshotTokens(context)
        appModel.serverBridge.connectRemoteSlingshotUrlServer(
            server.id,
            server.name,
            environment.connectionUrl,
            tokens.accessToken,
            tokens.accountId,
            stepUpToken,
        )
        SavedServerStore.remember(context, server.normalizedForPersistence())
        appModel.refreshSnapshot()
    }

    fun finishSuccessfulSlingshotConnect() {
        showSlingshotComputers = false
        onDismiss()
    }

    fun startSlingshotConnect(environment: AppSlingshotEnvironment) {
        if (!environment.online) {
            connectError = "${environment.displayName} is offline."
            return
        }
        scope.launch {
            var needsAuthorization = false
            try {
                connectSlingshotEnvironmentOrThrow(environment, "")
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                if (!ChatGPTOAuth.isRemoteControlAuthorizationRequired(e)) {
                    LLog.e(
                        logTag,
                        "slingshot cached connect failed",
                        e,
                        fields = mapOf("environmentId" to environment.id),
                    )
                    connectError = e.message ?: context.getString(R.string.discovery_connect_computer_failed)
                    return@launch
                }
                needsAuthorization = true
            }

            if (!needsAuthorization) {
                finishSuccessfulSlingshotConnect()
                return@launch
            }

            try {
                pendingSlingshotEnvironment = environment
                slingshotStepUpLauncher.launch(
                    ChatGPTOAuthActivity.createIntent(
                        context,
                        ChatGPTOAuth.createRemoteControlEnrollmentAttempt(),
                    ),
                )
            } catch (e: Exception) {
                pendingSlingshotEnvironment = null
                connectError = e.localizedMessage
                    ?: e.message
                    ?: context.getString(R.string.discovery_authorize_failed)
            }
        }
    }

    suspend fun connectSlingshotEnvironment(environment: AppSlingshotEnvironment, stepUpToken: String) {
        try {
            connectSlingshotEnvironmentOrThrow(environment, stepUpToken)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            LLog.e(
                logTag,
                "slingshot connect failed",
                e,
                fields = mapOf("environmentId" to environment.id),
            )
            connectError = e.message ?: context.getString(R.string.discovery_connect_computer_failed)
            return
        }
        finishSuccessfulSlingshotConnect()
    }

    LaunchedEffect(authorizedSlingshotConnect) {
        val pending = authorizedSlingshotConnect ?: return@LaunchedEffect
        authorizedSlingshotConnect = null
        connectSlingshotEnvironment(pending.first, pending.second)
    }

    suspend fun openSshSession(server: SavedServer, credential: SavedSshCredential): AppSshSessionResult =
        when (credential.method) {
            SshAuthMethod.PASSWORD -> appModel.ssh.sshOpenSession(
                host = server.hostname,
                port = server.resolvedSshPort.toUShort(),
                username = credential.username,
                password = credential.password,
                privateKeyPem = null,
                passphrase = null,
                unlockMacosKeychain = credential.unlockMacosKeychain,
                acceptUnknownHost = true,
            )

            SshAuthMethod.KEY -> appModel.ssh.sshOpenSession(
                host = server.hostname,
                port = server.resolvedSshPort.toUShort(),
                username = credential.username,
                password = null,
                privateKeyPem = credential.privateKey,
                passphrase = credential.passphrase,
                unlockMacosKeychain = false,
                acceptUnknownHost = true,
            )
        }

    suspend fun startGuidedSshConnect(server: SavedServer, credential: SavedSshCredential) {
        when (credential.method) {
            SshAuthMethod.PASSWORD -> {
                appModel.serverBridge.startRemoteOverSshConnect(
                    serverId = server.id,
                    displayName = server.name,
                    host = server.hostname,
                    port = server.resolvedSshPort.toUShort(),
                    username = credential.username,
                    password = credential.password,
                    privateKeyPem = null,
                    passphrase = null,
                    unlockMacosKeychain = credential.unlockMacosKeychain,
                    acceptUnknownHost = true,
                    workingDir = null,
                )
            }

            SshAuthMethod.KEY -> {
                appModel.serverBridge.startRemoteOverSshConnect(
                    serverId = server.id,
                    displayName = server.name,
                    host = server.hostname,
                    port = server.resolvedSshPort.toUShort(),
                    username = credential.username,
                    password = null,
                    privateKeyPem = credential.privateKey,
                    passphrase = credential.passphrase,
                    unlockMacosKeychain = false,
                    acceptUnknownHost = true,
                    workingDir = null,
                )
            }
        }
    }

    suspend fun prepareServerForSelection(entry: SavedServer): SavedServer {
        if (entry.websocketURL != null) {
            return entry
        }

        wakingServerId = entry.id
        try {
            return when (
                val probeResult = probeManualServer(
                    host = entry.hostname,
                    preferredCodexPort = entry.directCodexPort ?: entry.availableDirectCodexPorts.firstOrNull(),
                    timeoutMillis = 12_000L,
                )
            ) {
                is ManualServerProbeResult.Codex -> entry.copy(
                    port = probeResult.port,
                    codexPorts = listOf(probeResult.port) + entry.availableDirectCodexPorts.filter { it != probeResult.port },
                    hasCodexServer = true,
                    preferredConnectionMode = entry.preferredConnectionMode,
                    preferredCodexPort = probeResult.port,
                ).normalizedForPersistence()

                is ManualServerProbeResult.Ssh -> entry.copy(
                    port = probeResult.port,
                    sshPort = probeResult.port,
                    hasCodexServer = false,
                    preferredConnectionMode = "ssh",
                    preferredCodexPort = null,
                ).normalizedForPersistence()

                ManualServerProbeResult.None -> entry
            }
        } finally {
            wakingServerId = null
        }
    }

    suspend fun connectPreparedRemoteUrl(prepared: SavedServer) {
        val websocketURL = prepared.websocketURL ?: return
        appModel.serverBridge.connectRemoteUrlServer(
            prepared.id,
            prepared.name,
            websocketURL,
        )
    }

    suspend fun connectSelectedServer(entry: SavedServer) {
        if (wakingServerId != null && wakingServerId != entry.id) {
            return
        }

        try {
            val connected = connectedSnapshot(entry, snapshot?.servers ?: emptyList())
            if (connected?.isConnected == true) {
                LLog.t(logTag, "server already connected", fields = mapOf("serverId" to entry.id))
                onDismiss()
                return
            }

            val prepared = prepareServerForSelection(entry)
            when {
                prepared.websocketURL != null -> {
                    connectPreparedRemoteUrl(prepared)
                    SavedServerStore.remember(context, prepared.normalizedForPersistence())
                    appModel.refreshSnapshot()
                    onDismiss()
                }

                prepared.prefersSshConnection || (!prepared.hasCodexServer && prepared.canConnectViaSsh) -> {
                    sshServer = prepared.withPreferredConnection("ssh")
                }

                prepared.directCodexPort != null -> {
                    val directCodexPort = prepared.directCodexPort!!
                    appModel.serverBridge.connectRemoteServer(
                        prepared.id,
                        prepared.name,
                        prepared.hostname,
                        directCodexPort.toUShort(),
                    )
                    SavedServerStore.remember(
                        context,
                        prepared.withPreferredConnection("directCodex", directCodexPort),
                    )
                    appModel.refreshSnapshot()
                    onDismiss()
                }

                else -> {
                    connectError = context.getString(R.string.discovery_wake_failed)
                }
            }
        } catch (e: Exception) {
            LLog.e(
                logTag,
                "server connect failed",
                e,
                fields = mapOf(
                    "serverId" to entry.id,
                    "host" to entry.hostname,
                    "preferredConnectionMode" to entry.preferredConnectionMode,
                ),
            )
            connectError = e.message ?: context.getString(R.string.discovery_connect_failed)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = stringResource(R.string.discovery_add_server_heading),
                color = LitterTheme.textPrimary,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
        }

        Spacer(Modifier.height(8.dp))

        Text(
            text = stringResource(R.string.discovery_pick_connection),
            color = LitterTheme.textSecondary,
            fontSize = 12.sp,
        )

        Spacer(Modifier.height(14.dp))

        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            ChooserCard(
                title = stringResource(R.string.discovery_pair_kittylitter),
                subtitle = stringResource(R.string.discovery_scan_qr_subtitle),
                badge = "RECOMMENDED",
                icon = Icons.Default.QrCodeScanner,
                supportedAgents = KittylitterAgents,
                isRecommended = true,
                onClick = {
                    alleycatPairingMode = AlleycatPairingMode.Kittylitter
                    showAlleycatSheet = true
                },
            )

            ChooserCard(
                title = "Local Studio",
                subtitle = "Scan the QR from Local Studio Profile, or paste Copy connection JSON.",
                badge = null,
                icon = Icons.Outlined.DeveloperBoard,
                supportedAgents = listOf("local-studio"),
                isRecommended = false,
                onClick = {
                    alleycatPairingMode = AlleycatPairingMode.LocalStudio
                    showAlleycatSheet = true
                },
            )

            ChooserCard(
                title = stringResource(R.string.discovery_connected_computer),
                subtitle = stringResource(R.string.discovery_chatgpt_subtitle),
                badge = null,
                icon = Icons.Outlined.DesktopWindows,
                supportedAgents = CodexOnlyAgents,
                isRecommended = false,
                onClick = { showSlingshotComputers = true },
            )

            ChooserCard(
                title = stringResource(R.string.discovery_ssh_or_url),
                subtitle = stringResource(R.string.discovery_manual_subtitle),
                badge = null,
                icon = Icons.Outlined.Terminal,
                supportedAgents = CodexOnlyAgents,
                isRecommended = false,
                onClick = { showManualEntry = true },
            )
        }
    }

    if (showManualEntry) {
        ManualEntryDialog(
            onDismiss = { showManualEntry = false },
            onSubmit = { action ->
                when (action) {
                    is ManualEntryAction.Connect -> {
                        showManualEntry = false
                        scope.launch { connectSelectedServer(action.server) }
                    }

                    is ManualEntryAction.ContinueWithSsh -> {
                        pendingManualSshServer = action.server
                        showManualEntry = false
                    }
                }
            },
        )
    }

    if (showSlingshotComputers) {
        ConnectedComputersDialog(
            environments = slingshotEnvironments,
            loading = slingshotIsLoading,
            error = slingshotError,
            onDismiss = { showSlingshotComputers = false },
            onRefresh = { scope.launch { loadSlingshotEnvironments() } },
            onSelect = { environment ->
                startSlingshotConnect(environment)
            },
        )
        LaunchedEffect(Unit) {
            if (slingshotEnvironments.isEmpty() && !slingshotIsLoading) {
                loadSlingshotEnvironments()
            }
        }
    }

    sshServer?.let { server ->
        SSHLoginDialog(
            server = server,
            initialCredential = sshCredentialStore.load(server.hostname, server.resolvedSshPort),
            onDismiss = { sshServer = null },
            onConnect = { credential, rememberCredentials ->
                try {
                    LLog.t(
                        logTag,
                        "starting SSH connect",
                        fields = mapOf(
                            "serverId" to server.id,
                            "host" to server.hostname,
                            "sshPort" to server.resolvedSshPort,
                            "authMethod" to credential.method.name,
                            "os" to server.os,
                        ),
                    )
                    if (rememberCredentials) {
                        sshCredentialStore.save(server.hostname, server.resolvedSshPort, credential)
                    } else {
                        sshCredentialStore.delete(server.hostname, server.resolvedSshPort)
                    }

                    val session = openSshSession(server, credential)
                    val availability = appModel.ssh.sshProbeRemoteAgents(session.sessionId)
                    val bridgeAgents = availableSshBridgeKinds(availability)
                    if (bridgeAgents.isNotEmpty()) {
                        sshAgentContext = SshBridgeAgentContext(
                            server = server,
                            sessionId = session.sessionId,
                            host = session.normalizedHost,
                            availability = availability,
                            credential = credential,
                        )
                        sshServer = null
                        null
                    } else {
                        appModel.ssh.sshClose(session.sessionId)
                        LLog.t(
                            logTag,
                            "no SSH bridge agents available; falling back to Codex SSH",
                            fields = mapOf(
                                "serverId" to server.id,
                                "host" to server.hostname,
                            ),
                        )
                        startGuidedSshConnect(server, credential)
                        SavedServerStore.remember(
                            context,
                            server.withPreferredConnection("ssh"),
                        )
                        appModel.refreshSnapshot()
                        pendingAutoNavigateServerId = server.id
                        LLog.t(
                            logTag,
                            "guided SSH bootstrap started",
                            fields = mapOf(
                                "serverId" to server.id,
                                "host" to server.hostname,
                                "sshPort" to server.resolvedSshPort,
                            ),
                        )
                        sshServer = null
                        null
                    }
                } catch (e: Exception) {
                    LLog.e(
                        logTag,
                        "guided SSH connect failed",
                        e,
                        fields = mapOf(
                            "serverId" to server.id,
                            "host" to server.hostname,
                            "sshPort" to server.resolvedSshPort,
                            "authMethod" to credential.method.name,
                            "os" to server.os,
                        ),
                    )
                    e.message ?: context.getString(R.string.discovery_ssh_connect_failed)
                }
            },
        )
    }

    sshAgentContext?.let { agentContext ->
        SSHAgentPickerDialog(
            context = agentContext,
            onDismiss = {
                scope.launch {
                    runCatching { appModel.ssh.sshClose(agentContext.sessionId) }
                    sshAgentContext = null
                }
            },
            onUseCodex = {
                scope.launch {
                    runCatching { appModel.ssh.sshClose(agentContext.sessionId) }
                    startGuidedSshConnect(agentContext.server, agentContext.credential)
                    SavedServerStore.remember(
                        context,
                        agentContext.server.withPreferredConnection("ssh"),
                    )
                    appModel.refreshSnapshot()
                    pendingAutoNavigateServerId = agentContext.server.id
                    sshAgentContext = null
                }
            },
            onConnect = { selectedKinds ->
                try {
                    val result = appModel.ssh.sshConnectBridgeSession(
                        sessionId = agentContext.sessionId,
                        serverId = "ssh-bridge:${agentContext.host}",
                        displayName = agentContext.server.name,
                        host = agentContext.host,
                        stateRoot = sshBridgeStateRoot(context, agentContext.host),
                        runtimeKinds = selectedKinds,
                        transport = SshBridgeTransport.EPHEMERAL,
                    )
                    val server = agentContext.server.copy(
                        id = result.serverId,
                        hostname = agentContext.host,
                        port = 0,
                        codexPorts = emptyList(),
                        source = "ssh",
                        hasCodexServer = true,
                        preferredConnectionMode = "ssh",
                        alleycatAgentName = selectedKinds.joinToString(","),
                        alleycatAgentWire = "ssh-bridge",
                    )
                    appModel.sshSessionStore.record(result.serverId, agentContext.sessionId)
                    SavedServerStore.remember(context, server)
                    appModel.refreshSnapshot()
                    pendingAutoNavigateServerId = result.serverId
                    sshAgentContext = null
                    null
                } catch (e: Exception) {
                    LLog.e(
                        logTag,
                        "SSH bridge connect failed",
                        e,
                        fields = mapOf(
                            "serverId" to agentContext.server.id,
                            "host" to agentContext.host,
                        ),
                    )
                    e.message ?: context.getString(R.string.discovery_ssh_agents_failed)
                }
            },
        )
    }

    connectError?.let { message ->
        AlertDialog(
            onDismissRequest = { connectError = null },
            title = { Text(stringResource(R.string.discovery_connection_failed)) },
            text = { Text(message) },
            confirmButton = {
                TextButton(onClick = { connectError = null }) {
                    Text(stringResource(R.string.common_ok))
                }
            },
        )
    }

    if (showAlleycatSheet) {
        @OptIn(ExperimentalMaterial3Api::class)
        ModalBottomSheet(
            onDismissRequest = { showAlleycatSheet = false },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = LitterTheme.background,
        ) {
            AlleycatAddServerSheet(
                onDismiss = { showAlleycatSheet = false },
                startScanningOnAppear = true,
                pairingMode = alleycatPairingMode,
                onConnected = { result ->
                    showAlleycatSheet = false
                    scope.launch {
                        SavedServerStore.rememberAlleycat(
                            context = context,
                            serverId = result.serverId,
                            displayName = result.displayName,
                            nodeId = result.nodeId,
                            relay = result.params.relay,
                            agentName = result.agentName,
                            agentWire = alleycatWireStorageValue(result.agentWire),
                        )
                        appModel.refreshSnapshot()
                        pendingAutoNavigateServerId = result.serverId
                    }
                },
            )
        }
    }
}

/**
 * Canonical agent list shown on the kittylitter chooser card. Mirrors
 * the splash carousel order so cold-start branding stays consistent.
 * New agents added in the alleycat manifest still surface on connected
 * hosts via the real metadata store; this list only seeds the
 * pre-pair preview.
 */
private val KittylitterAgents: List<AgentRuntimeKind> = listOf(
    "codex",
    "pi",
    "amp",
    "opencode",
    "claude",
    "droid",
    "hermes",
    "devin",
    "grok",
)

private val CodexOnlyAgents: List<AgentRuntimeKind> = listOf("codex")

@Composable
private fun ChooserCard(
    title: String,
    subtitle: String,
    badge: String?,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    supportedAgents: List<AgentRuntimeKind>,
    isRecommended: Boolean,
    onClick: () -> Unit,
) {
    val borderColor = if (isRecommended) {
        LitterTheme.accent.copy(alpha = 0.45f)
    } else {
        LitterTheme.accent.copy(alpha = 0.18f)
    }
    val backgroundColor = if (isRecommended) {
        LitterTheme.surface.copy(alpha = 0.85f)
    } else {
        LitterTheme.surface.copy(alpha = 0.6f)
    }
    val iconBubble = LitterTheme.accent.copy(alpha = if (isRecommended) 0.16f else 0.10f)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(backgroundColor, RoundedCornerShape(14.dp))
            .border(
                width = if (isRecommended) 1.dp else 0.8.dp,
                color = borderColor,
                shape = RoundedCornerShape(14.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            verticalAlignment = Alignment.Top,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Box(
                modifier = Modifier
                    .padding(top = 2.dp)
                    .size(36.dp)
                    .background(iconBubble, RoundedCornerShape(50)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = LitterTheme.accent,
                    modifier = Modifier.size(20.dp),
                )
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = title,
                        color = LitterTheme.textPrimary,
                        fontSize = 15.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                    if (badge != null) {
                        Box(
                            modifier = Modifier
                                .background(
                                    LitterTheme.accent.copy(alpha = 0.14f),
                                    RoundedCornerShape(50),
                                )
                                .border(
                                    width = 0.6.dp,
                                    color = LitterTheme.accent.copy(alpha = 0.45f),
                                    shape = RoundedCornerShape(50),
                                )
                                .padding(horizontal = 6.dp, vertical = 2.dp),
                        ) {
                            Text(
                                text = badge,
                                color = LitterTheme.accent,
                                fontSize = 10.sp,
                                fontWeight = FontWeight.SemiBold,
                                letterSpacing = 0.5.sp,
                            )
                        }
                    }
                }
                Text(
                    text = subtitle,
                    color = LitterTheme.textSecondary,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
            Icon(
                imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = LitterTheme.textMuted,
                modifier = Modifier.padding(top = 10.dp),
            )
        }

        if (supportedAgents.isNotEmpty()) {
            SupportedAgentsStrip(supportedAgents)
        }
    }
}

@Composable
private fun SupportedAgentsStrip(agents: List<AgentRuntimeKind>) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = stringResource(R.string.discovery_works_with),
            color = LitterTheme.textMuted,
            fontSize = 10.sp,
            letterSpacing = 0.4.sp,
            maxLines = 1,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
            agents.forEach { agent ->
                com.litter.android.ui.common.AgentIconView(
                    kind = agent,
                    sizeDp = 18,
                )
            }
        }
    }
}

@Composable
private fun ServerRow(
    entry: SavedServer,
    connectedServer: AppServerSnapshot?,
    isWaking: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
    onRename: (() -> Unit)?,
) {
    val displayHost = connectedServer?.host ?: entry.hostname
    val subtitle = connectedServer?.connectionProgressDetail
        ?: buildString {
            append(displayHost)
            if (entry.os != null) {
                append(" - ")
                append(entry.os)
            }
            if (entry.availableDirectCodexPorts.isNotEmpty()) {
                append(" - codex ")
                append(entry.availableDirectCodexPorts.joinToString(", "))
            }
            if (entry.canConnectViaSsh) {
                append(" - ssh ")
                append(entry.resolvedSshPort)
            }
            if (entry.wakeMAC != null) {
                append(" - wake")
            }
        }
    val serverIcon = serverIconForEntry(entry)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(LitterTheme.surface, RoundedCornerShape(10.dp))
            .clickable(enabled = enabled, onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = serverIcon,
            contentDescription = entry.os ?: entry.source,
            tint = if (entry.hasCodexServer) LitterTheme.accent else LitterTheme.textMuted,
            modifier = Modifier.size(20.dp),
        )
        Spacer(Modifier.width(10.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(entry.name.ifBlank { entry.hostname }, color = LitterTheme.textPrimary, fontSize = 14.sp)
            Text(subtitle, color = LitterTheme.textSecondary, fontSize = 11.sp)
        }
        val (sourceColor, sourceLabel) = when (entry.source) {
            "bonjour" -> LitterTheme.info to "Bonjour"
            "tailscale" -> Color(0xFFC797D8) to "Tailscale"
            "lanProbe" -> LitterTheme.accent to "LAN"
            "arpScan" -> LitterTheme.textSecondary to "ARP"
            "ssh" -> Color(0xFFFF9500) to "SSH"
            "local" -> LitterTheme.accent to "Local"
            else -> LitterTheme.textMuted to "Manual"
        }
        Text(
            text = sourceLabel,
            color = sourceColor,
            fontSize = 10.sp,
            modifier = Modifier
                .background(sourceColor.copy(alpha = 0.12f), RoundedCornerShape(4.dp))
                .padding(horizontal = 6.dp, vertical = 2.dp),
        )
        if (connectedServer != null && connectedServer.health != AppServerHealth.DISCONNECTED) {
            Spacer(Modifier.width(6.dp))
            Text(
                text = connectedServer.statusLabel,
                color = connectedServer.statusColor,
                fontSize = 10.sp,
                modifier = Modifier
                    .background(connectedServer.statusColor.copy(alpha = 0.12f), RoundedCornerShape(4.dp))
                    .padding(horizontal = 6.dp, vertical = 2.dp),
            )
        }
        if (isWaking) {
            Spacer(Modifier.width(6.dp))
            CircularProgressIndicator(
                modifier = Modifier.size(14.dp),
                strokeWidth = 2.dp,
                color = LitterTheme.accent,
            )
        }
        if (onRename != null) {
            Spacer(Modifier.width(2.dp))
            IconButton(
                onClick = onRename,
                enabled = enabled,
                modifier = Modifier.size(28.dp),
            ) {
                Icon(
                    imageVector = Icons.Outlined.Edit,
                    contentDescription = stringResource(R.string.settings_rename_server),
                    tint = LitterTheme.textMuted,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
    }
}

@Composable
private fun ConnectedComputersDialog(
    environments: List<AppSlingshotEnvironment>,
    loading: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onRefresh: () -> Unit,
    onSelect: (AppSlingshotEnvironment) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.discovery_connected_computers)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = stringResource(R.string.discovery_computers_helper),
                    color = LitterTheme.textSecondary,
                    fontSize = 12.sp,
                )
                when {
                    loading && environments.isEmpty() -> {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(18.dp),
                                strokeWidth = 2.dp,
                                color = LitterTheme.accent,
                            )
                            Text(
                                text = stringResource(R.string.discovery_loading_computers),
                                color = LitterTheme.textSecondary,
                                fontSize = 12.sp,
                            )
                        }
                    }

                    error != null -> {
                        Text(
                            text = error,
                            color = LitterTheme.danger,
                            fontSize = 12.sp,
                        )
                    }

                    environments.isEmpty() -> {
                        Text(
                            text = stringResource(R.string.discovery_no_computers),
                            color = LitterTheme.textSecondary,
                            fontSize = 12.sp,
                        )
                    }

                    else -> {
                        if (loading) {
                            LinearProgressIndicator(
                                modifier = Modifier.fillMaxWidth(),
                                color = LitterTheme.accent,
                                trackColor = LitterTheme.border,
                            )
                        }
                        LazyColumn(
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.height(340.dp),
                        ) {
                            items(environments, key = { it.id }) { environment ->
                                ConnectedComputerRow(
                                    environment = environment,
                                    onClick = { onSelect(environment) },
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = onRefresh,
                enabled = !loading,
            ) {
                Text(stringResource(R.string.common_refresh))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun ConnectedComputerRow(
    environment: AppSlingshotEnvironment,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier
            .fillMaxWidth()
            .background(LitterTheme.surface, RoundedCornerShape(10.dp))
            .clickable(enabled = environment.online, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
    ) {
        Icon(
            imageVector = slingshotEnvironmentIcon(environment),
            contentDescription = null,
            tint = if (environment.online) LitterTheme.accent else LitterTheme.textMuted,
            modifier = Modifier.size(22.dp),
        )
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = environment.displayName,
                color = if (environment.online) LitterTheme.textPrimary else LitterTheme.textSecondary,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = slingshotEnvironmentSubtitle(environment),
                color = LitterTheme.textSecondary,
                fontSize = 11.sp,
            )
        }
        Text(
            text = slingshotEnvironmentStatus(environment),
            color = if (environment.online && !environment.busy) LitterTheme.accent else LitterTheme.textMuted,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

private fun slingshotSavedServer(environment: AppSlingshotEnvironment): SavedServer =
    SavedServer(
        id = "slingshot-${environment.id}",
        name = environment.displayName,
        hostname = environment.id,
        port = 0,
        codexPorts = emptyList(),
        source = "manual",
        hasCodexServer = true,
        preferredConnectionMode = "directCodex",
        websocketURL = environment.connectionUrl,
        os = environment.operatingSystem,
        rememberedByUser = true,
    )

private fun slingshotEnvironmentSubtitle(environment: AppSlingshotEnvironment): String {
    val parts = buildList {
        environment.hostName?.trim()?.takeIf { it.isNotEmpty() }?.let(::add)
        listOfNotNull(
            environment.operatingSystem.trim().takeIf { it.isNotEmpty() },
            environment.architecture?.trim()?.takeIf { it.isNotEmpty() },
        ).joinToString(" ").takeIf { it.isNotEmpty() }?.let(::add)
        environment.appServerVersion?.trim()?.takeIf { it.isNotEmpty() }?.let { add("Codex $it") }
    }
    return parts.ifEmpty { listOf(environment.id) }.joinToString(" - ")
}

private fun slingshotEnvironmentStatus(environment: AppSlingshotEnvironment): String =
    when {
        !environment.online -> "offline"
        environment.busy -> "busy"
        else -> "online"
    }

private fun slingshotEnvironmentIcon(
    environment: AppSlingshotEnvironment,
): androidx.compose.ui.graphics.vector.ImageVector =
    when (environment.operatingSystem.lowercase()) {
        "linux" -> Icons.Outlined.Dns
        "windows" -> Icons.Outlined.DesktopWindows
        "macos", "darwin" -> Icons.Outlined.DesktopWindows
        else -> Icons.Outlined.Laptop
    }

@Composable
private fun ManualEntryDialog(
    onDismiss: () -> Unit,
    onSubmit: (ManualEntryAction) -> Unit,
) {
    val context = LocalContext.current
    var mode by remember { mutableStateOf(ManualConnectionMode.SSH) }
    var codexUrl by remember { mutableStateOf("") }
    var host by remember { mutableStateOf("") }
    var sshPort by remember { mutableStateOf("22") }
    var wakeMac by remember { mutableStateOf("") }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.discovery_add_server)) },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(12.dp),
                modifier = Modifier.verticalScroll(rememberScrollState()),
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = mode == ManualConnectionMode.CODEX,
                        onClick = { mode = ManualConnectionMode.CODEX },
                        label = { Text(stringResource(ManualConnectionMode.CODEX.labelRes)) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = LitterTheme.accent.copy(alpha = 0.18f),
                            selectedLabelColor = LitterTheme.textPrimary,
                        ),
                    )
                    FilterChip(
                        selected = mode == ManualConnectionMode.SSH,
                        onClick = { mode = ManualConnectionMode.SSH },
                        label = { Text(stringResource(ManualConnectionMode.SSH.labelRes)) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = LitterTheme.accent.copy(alpha = 0.18f),
                            selectedLabelColor = LitterTheme.textPrimary,
                        ),
                    )
                }

                when (mode) {
                    ManualConnectionMode.CODEX -> {
                        OutlinedTextField(
                            value = codexUrl,
                            onValueChange = {
                                codexUrl = it
                                errorMessage = null
                            },
                            label = { Text(stringResource(R.string.discovery_codex_url)) },
                            placeholder = { Text(stringResource(R.string.discovery_codex_url_placeholder)) },
                            singleLine = true,
                        )
                        Text(
                            text = stringResource(R.string.discovery_manual_security_help),
                            color = LitterTheme.textMuted,
                            fontSize = 11.sp,
                        )
                    }

                    ManualConnectionMode.SSH -> {
                        OutlinedTextField(
                            value = host,
                            onValueChange = {
                                host = it
                                errorMessage = null
                            },
                            label = { Text(stringResource(R.string.discovery_ssh_host)) },
                            placeholder = { Text(stringResource(R.string.discovery_hostname_or_ip)) },
                            singleLine = true,
                        )
                        OutlinedTextField(
                            value = sshPort,
                            onValueChange = {
                                sshPort = it
                                errorMessage = null
                            },
                            label = { Text(stringResource(R.string.discovery_ssh_port)) },
                            singleLine = true,
                        )
                        OutlinedTextField(
                            value = wakeMac,
                            onValueChange = {
                                wakeMac = it
                                errorMessage = null
                            },
                            label = { Text(stringResource(R.string.discovery_wake_mac_optional)) },
                            placeholder = { Text("aa:bb:cc:dd:ee:ff") },
                            singleLine = true,
                        )
                    }
                }

                if (errorMessage != null) {
                    Text(
                        text = errorMessage!!,
                        color = LitterTheme.danger,
                        fontSize = 12.sp,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    errorMessage = when (val action = buildManualEntryAction(
                        context,
                        mode,
                        codexUrl,
                        host,
                        sshPort,
                        wakeMac,
                    )) {
                        is ManualEntryBuild.Action -> {
                            onSubmit(action.action)
                            null
                        }

                        is ManualEntryBuild.Error -> action.message
                    }
                },
            ) {
                Text(stringResource(mode.primaryButtonTitleRes))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun RenameServerDialog(
    server: SavedServer,
    onDismiss: () -> Unit,
    onRename: (String) -> Unit,
) {
    var newName by remember(server.id) {
        mutableStateOf(server.name.ifBlank { server.hostname })
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.settings_rename_server)) },
        text = {
            OutlinedTextField(
                value = newName,
                onValueChange = { newName = it },
                label = { Text(stringResource(R.string.settings_name)) },
                singleLine = true,
            )
        },
        confirmButton = {
            TextButton(onClick = { onRename(newName.trim()) }) {
                Text(stringResource(R.string.common_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
internal fun SSHLoginDialog(
    server: SavedServer,
    initialCredential: SavedSshCredential?,
    onDismiss: () -> Unit,
    onConnect: suspend (SavedSshCredential, Boolean) -> String?,
) {
    val scope = rememberCoroutineScope()
    var username by remember(server.id) { mutableStateOf(initialCredential?.username ?: "") }
    var authMethod by remember(server.id) { mutableStateOf(initialCredential?.method ?: SshAuthMethod.PASSWORD) }
    var password by remember(server.id) { mutableStateOf(initialCredential?.password ?: "") }
    var isPasswordVisible by remember(server.id) { mutableStateOf(false) }
    var privateKey by remember(server.id) { mutableStateOf(initialCredential?.privateKey ?: "") }
    var passphrase by remember(server.id) { mutableStateOf(initialCredential?.passphrase ?: "") }
    var rememberCredentials by remember(server.id) { mutableStateOf(initialCredential != null) }
    var unlockMacosKeychain by remember(server.id) {
        mutableStateOf(initialCredential?.unlockMacosKeychain ?: false)
    }
    var isConnecting by remember(server.id) { mutableStateOf(false) }
    var errorMessage by remember(server.id) { mutableStateOf<String?>(null) }
    val hostDisplay = if (server.resolvedSshPort == 22) {
        server.hostname
    } else {
        "${server.hostname}:${server.resolvedSshPort}"
    }

    AlertDialog(
        onDismissRequest = { if (!isConnecting) onDismiss() },
        title = { Text(stringResource(R.string.discovery_ssh_login)) },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier.verticalScroll(rememberScrollState()),
            ) {
                Text(
                    text = "${server.name.ifBlank { server.hostname }}\n$hostDisplay",
                    color = LitterTheme.textPrimary,
                    fontSize = 13.sp,
                )
                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it },
                    label = { Text(stringResource(R.string.discovery_username)) },
                    singleLine = true,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(
                        onClick = { authMethod = SshAuthMethod.PASSWORD },
                        enabled = !isConnecting,
                    ) {
                        Text(
                            stringResource(
                                if (authMethod == SshAuthMethod.PASSWORD) {
                                    R.string.discovery_password_required
                                } else {
                                    R.string.discovery_password
                                },
                            ),
                        )
                    }
                    TextButton(
                        onClick = {
                            authMethod = SshAuthMethod.KEY
                            isPasswordVisible = false
                        },
                        enabled = !isConnecting,
                    ) {
                        Text(
                            stringResource(
                                if (authMethod == SshAuthMethod.KEY) {
                                    R.string.discovery_ssh_key_required
                                } else {
                                    R.string.discovery_ssh_key
                                },
                            ),
                        )
                    }
                }
                if (authMethod == SshAuthMethod.PASSWORD) {
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text(stringResource(R.string.discovery_password)) },
                        singleLine = true,
                        visualTransformation = if (isPasswordVisible) {
                            VisualTransformation.None
                        } else {
                            PasswordVisualTransformation()
                        },
                        trailingIcon = {
                            IconButton(
                                onClick = { isPasswordVisible = !isPasswordVisible },
                                enabled = !isConnecting,
                            ) {
                                Icon(
                                    imageVector = if (isPasswordVisible) {
                                        Icons.Filled.VisibilityOff
                                    } else {
                                        Icons.Filled.Visibility
                                    },
                                    contentDescription = stringResource(
                                        if (isPasswordVisible) {
                                            R.string.discovery_hide_password
                                        } else {
                                            R.string.discovery_show_password
                                        },
                                    ),
                                )
                            }
                        },
                    )
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Switch(
                            checked = unlockMacosKeychain,
                            onCheckedChange = { unlockMacosKeychain = it },
                            enabled = !isConnecting,
                        )
                        Column {
                            Text(
                                text = stringResource(R.string.discovery_unlock_keychain),
                                color = LitterTheme.textPrimary,
                                fontSize = 12.sp,
                            )
                            Text(
                                text = stringResource(R.string.discovery_unlock_keychain_help),
                                color = LitterTheme.textSecondary,
                                fontSize = 11.sp,
                            )
                        }
                    }
                } else {
                    OutlinedTextField(
                        value = privateKey,
                        onValueChange = { privateKey = it },
                        label = { Text(stringResource(R.string.discovery_private_key)) },
                        minLines = 5,
                    )
                    OutlinedTextField(
                        value = passphrase,
                        onValueChange = { passphrase = it },
                        label = { Text(stringResource(R.string.discovery_passphrase_optional)) },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                    )
                }
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Switch(
                        checked = rememberCredentials,
                        onCheckedChange = { rememberCredentials = it },
                        enabled = !isConnecting,
                    )
                    Text(
                        text = stringResource(R.string.discovery_remember_credentials),
                        color = LitterTheme.textSecondary,
                        fontSize = 12.sp,
                    )
                }
                if (errorMessage != null) {
                    Text(
                        text = errorMessage!!,
                        color = LitterTheme.danger,
                        fontSize = 12.sp,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !isConnecting && username.isNotBlank() && when (authMethod) {
                    SshAuthMethod.PASSWORD -> password.isNotBlank()
                    SshAuthMethod.KEY -> privateKey.isNotBlank()
                },
                onClick = {
                    val credential = when (authMethod) {
                        SshAuthMethod.PASSWORD -> SavedSshCredential(
                            username = username.trim(),
                            method = SshAuthMethod.PASSWORD,
                            password = password,
                            unlockMacosKeychain = unlockMacosKeychain,
                        )

                        SshAuthMethod.KEY -> SavedSshCredential(
                            username = username.trim(),
                            method = SshAuthMethod.KEY,
                            privateKey = privateKey,
                            passphrase = passphrase.ifBlank { null },
                            unlockMacosKeychain = false,
                        )
                    }
                    scope.launch {
                        isConnecting = true
                        errorMessage = onConnect(credential, rememberCredentials)
                        isConnecting = false
                    }
                },
            ) {
                if (isConnecting) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(14.dp),
                        strokeWidth = 2.dp,
                        color = LitterTheme.accent,
                    )
                } else {
                    Text(stringResource(R.string.common_connect))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !isConnecting) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun SSHAgentPickerDialog(
    context: SshBridgeAgentContext,
    onDismiss: () -> Unit,
    onUseCodex: () -> Unit,
    onConnect: suspend (List<AgentRuntimeKind>) -> String?,
) {
    val androidContext = LocalContext.current
    val scope = rememberCoroutineScope()
    val availableKinds = remember(context.sessionId) {
        availableSshBridgeKinds(context.availability)
    }
    var selectedKinds by remember(context.sessionId) {
        mutableStateOf(availableKinds.toSet())
    }
    var isConnecting by remember(context.sessionId) { mutableStateOf(false) }
    var errorMessage by remember(context.sessionId) { mutableStateOf<String?>(null) }

    AlertDialog(
        onDismissRequest = { if (!isConnecting) onDismiss() },
        title = { Text(stringResource(R.string.discovery_remote_agents)) },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier.verticalScroll(rememberScrollState()),
            ) {
                Text(
                    text = "${context.server.name.ifBlank { context.host }}\n${context.host}",
                    color = LitterTheme.textPrimary,
                    fontSize = 13.sp,
                )
                context.availability.forEach { agent ->
                    val enabled = isSshBridgeKind(agent.kind) &&
                        agent.status == AgentAvailabilityStatus.AVAILABLE &&
                        !isConnecting
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(enabled = enabled) {
                                selectedKinds = if (agent.kind in selectedKinds) {
                                    selectedKinds - agent.kind
                                } else {
                                    selectedKinds + agent.kind
                                }
                            }
                            .padding(vertical = 4.dp),
                    ) {
                        AgentIconView(
                            kind = agent.kind,
                            sizeDp = 22,
                            modifier = Modifier.alpha(
                                if (agent.status == AgentAvailabilityStatus.AVAILABLE) 1f else 0.45f,
                            ),
                        )
                        Spacer(Modifier.width(10.dp))
                        Column(modifier = Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(
                                    text = sshRuntimeLabel(agent.kind),
                                    color = if (agent.status == AgentAvailabilityStatus.AVAILABLE) {
                                        LitterTheme.textPrimary
                                    } else {
                                        LitterTheme.textSecondary
                                    },
                                    fontSize = 14.sp,
                                    fontWeight = FontWeight.Medium,
                                )
                                if (agent.kind.isBeta) {
                                    Spacer(Modifier.width(6.dp))
                                    BetaBadge()
                                }
                            }
                            Text(
                                text = sshAgentStatusLabel(androidContext, agent),
                                color = LitterTheme.textSecondary,
                                fontSize = 11.sp,
                            )
                        }
                        if (agent.kind in selectedKinds) {
                            Icon(
                                imageVector = Icons.Filled.CheckCircle,
                                contentDescription = null,
                                tint = LitterTheme.accent,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
                if (errorMessage != null) {
                    Text(
                        text = errorMessage!!,
                        color = LitterTheme.danger,
                        fontSize = 12.sp,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !isConnecting && selectedKinds.isNotEmpty(),
                onClick = {
                    scope.launch {
                        isConnecting = true
                        errorMessage = onConnect(selectedKinds.sortedBy(::sshRuntimeSortRank))
                        isConnecting = false
                    }
                },
            ) {
                if (isConnecting) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(14.dp),
                        strokeWidth = 2.dp,
                        color = LitterTheme.accent,
                    )
                } else {
                    Text(stringResource(R.string.common_connect))
                }
            }
        },
        dismissButton = {
            Row {
                TextButton(onClick = onUseCodex, enabled = !isConnecting) {
                    Text(stringResource(R.string.discovery_use_codex_ssh))
                }
                TextButton(onClick = onDismiss, enabled = !isConnecting) {
                    Text(stringResource(R.string.common_cancel))
                }
            }
        },
    )
}

private fun availableSshBridgeKinds(agents: List<RemoteAgentAvailability>): List<AgentRuntimeKind> =
    agents
        .filter { isSshBridgeKind(it.kind) && it.status == AgentAvailabilityStatus.AVAILABLE }
        .map { it.kind }
        .sortedBy(::sshRuntimeSortRank)

private fun isSshBridgeKind(kind: AgentRuntimeKind): Boolean =
    kind.metadata?.capabilities?.supportsSshBridge
        ?: (kind in setOf("claude", "pi", "opencode", "local-studio"))

private fun sshRuntimeLabel(kind: AgentRuntimeKind): String = kind.runtimeLabel

private fun sshRuntimeSortRank(kind: AgentRuntimeKind): Int = kind.runtimeSortIndex

private fun sshAgentStatusLabel(context: Context, agent: RemoteAgentAvailability): String = when (agent.status) {
    AgentAvailabilityStatus.AVAILABLE -> context.getString(R.string.common_available)
    AgentAvailabilityStatus.AGENT_CLI_MISSING -> context.getString(R.string.discovery_cli_missing)
    AgentAvailabilityStatus.WINDOWS_NOT_YET_SUPPORTED ->
        context.getString(R.string.discovery_windows_unsupported)
}

private fun sshBridgeStateRoot(context: Context, host: String): String {
    val safeHost = host.replace(Regex("[^A-Za-z0-9._-]"), "_")
    val dir = File(File(context.filesDir, "alleycat-bridges"), safeHost)
    dir.mkdirs()
    return dir.absolutePath
}

private fun connectedSnapshot(
    entry: SavedServer,
    servers: List<AppServerSnapshot>,
): AppServerSnapshot? = servers.firstOrNull { it.serverId == entry.id }
    ?: servers.firstOrNull { it.host.lowercase().trim().trimStart('[').trimEnd(']') == entry.deduplicationKey }

private fun serverIconForEntry(entry: SavedServer): androidx.compose.ui.graphics.vector.ImageVector {
    if (entry.source == "local") return Icons.Outlined.PhoneAndroid
    val os = entry.os?.lowercase()
    if (os != null) {
        if (os.contains("windows")) return Icons.Outlined.DesktopWindows
        if (os.contains("raspbian")) return Icons.Outlined.DeveloperBoard
        if (
            os.contains("ubuntu") ||
            os.contains("debian") ||
            os.contains("fedora") ||
            os.contains("red hat") ||
            os.contains("freebsd") ||
            os.contains("linux")
        ) {
            return Icons.Outlined.Dns
        }
    }
    return when (entry.source) {
        "bonjour" -> Icons.Outlined.Laptop
        "tailscale" -> Icons.Outlined.Lan
        "ssh" -> Icons.Outlined.Terminal
        else -> Icons.Outlined.Dns
    }
}

private fun connectionChoiceMessage(context: Context, server: SavedServer): String {
    val directPorts = server.availableDirectCodexPorts.map(Int::toString)
    if (directPorts.isEmpty()) {
        return context.getString(R.string.discovery_use_ssh_host, server.hostname)
    }
    if (server.canConnectViaSsh) {
        return context.getString(
            R.string.discovery_direct_and_ssh_ports,
            directPorts.joinToString(", "),
            server.resolvedSshPort,
        )
    }
    return context.getString(R.string.discovery_choose_codex_port_host, server.hostname)
}

private sealed interface ManualEntryAction {
    data class Connect(val server: SavedServer) : ManualEntryAction
    data class ContinueWithSsh(val server: SavedServer) : ManualEntryAction
}

private sealed interface ManualEntryBuild {
    data class Action(val action: ManualEntryAction) : ManualEntryBuild
    data class Error(val message: String) : ManualEntryBuild
}

private enum class ManualConnectionMode(
    val labelRes: Int,
    val primaryButtonTitleRes: Int,
) {
    CODEX(R.string.settings_connection_codex, R.string.common_connect),
    SSH(R.string.settings_connection_ssh, R.string.discovery_continue_ssh_login),
}

private fun buildManualEntryAction(
    context: Context,
    mode: ManualConnectionMode,
    codexUrl: String,
    host: String,
    sshPort: String,
    wakeMac: String,
): ManualEntryBuild = when (mode) {
    ManualConnectionMode.CODEX -> buildManualCodexEntry(context, codexUrl)
    ManualConnectionMode.SSH -> buildManualSshEntry(context, host, sshPort, wakeMac)
}

private fun buildManualCodexEntry(context: Context, rawInput: String): ManualEntryBuild {
    val raw = rawInput.trim()
    if (raw.isEmpty()) {
        return ManualEntryBuild.Error(context.getString(R.string.discovery_enter_ws_url))
    }

    runCatching { URI(raw) }
        .getOrNull()
        ?.let { uri ->
            val scheme = uri.scheme?.lowercase()
            val host = uri.host?.takeIf { it.isNotBlank() }
            if ((scheme == "ws" || scheme == "wss") && host != null) {
                val port = uri.port.takeIf { it > 0 }
                return ManualEntryBuild.Action(
                    ManualEntryAction.Connect(
                        SavedServer(
                            id = "manual-url-$raw",
                            name = host,
                            hostname = host,
                            port = port ?: 0,
                            codexPorts = port?.let(::listOf) ?: emptyList(),
                            source = "manual",
                            hasCodexServer = true,
                            preferredConnectionMode = "directCodex",
                            preferredCodexPort = port,
                            websocketURL = raw,
                        ).normalizedForPersistence(),
                    ),
                )
            }
        }

    val (host, port) = parseBareHostAndPort(raw)
        ?: return ManualEntryBuild.Error(context.getString(R.string.discovery_enter_ws_url))
    if (host.isBlank()) {
        return ManualEntryBuild.Error(context.getString(R.string.discovery_enter_host))
    }

    return ManualEntryBuild.Action(
        ManualEntryAction.Connect(
            SavedServer(
                id = "manual-$host:$port",
                name = host,
                hostname = host,
                port = port,
                codexPorts = listOf(port),
                source = "manual",
                hasCodexServer = true,
                preferredConnectionMode = "directCodex",
                preferredCodexPort = port,
            ).normalizedForPersistence(),
        ),
    )
}

private fun buildManualSshEntry(
    context: Context,
    hostInput: String,
    sshPortInput: String,
    wakeMacInput: String,
): ManualEntryBuild {
    val host = hostInput.trim()
    if (host.isEmpty()) {
        return ManualEntryBuild.Error(context.getString(R.string.discovery_enter_host))
    }

    val sshPort = sshPortInput.trim().toIntOrNull()
    if (sshPort == null || sshPort !in 1..65535) {
        return ManualEntryBuild.Error(context.getString(R.string.discovery_invalid_ssh_port))
    }

    val wakeInput = wakeMacInput.trim()
    val normalizedWakeMac = SavedServer.normalizeWakeMac(wakeInput)
    if (wakeInput.isNotEmpty() && normalizedWakeMac == null) {
        return ManualEntryBuild.Error(context.getString(R.string.discovery_invalid_wake_mac))
    }

    return ManualEntryBuild.Action(
        ManualEntryAction.ContinueWithSsh(
            SavedServer(
                id = "manual-ssh-$host:$sshPort",
                name = host,
                hostname = host,
                port = sshPort,
                sshPort = sshPort,
                source = "manual",
                hasCodexServer = false,
                wakeMAC = normalizedWakeMac,
                preferredConnectionMode = "ssh",
            ).normalizedForPersistence(),
        ),
    )
}

private fun parseBareHostAndPort(raw: String): Pair<String, Int>? {
    if (raw.startsWith("[")) {
        val closing = raw.indexOf(']')
        if (closing > 1) {
            val host = raw.substring(1, closing)
            val portPart = raw.substring(closing + 1)
            val port = when {
                portPart.isEmpty() -> 8390
                portPart.startsWith(":") -> portPart.drop(1).toIntOrNull() ?: return null
                else -> return null
            }
            return host to port
        }
    }

    val colonCount = raw.count { it == ':' }
    if (colonCount == 1) {
        val index = raw.lastIndexOf(':')
        val host = raw.substring(0, index)
        val port = raw.substring(index + 1).toIntOrNull() ?: return null
        return host to port
    }

    return raw to 8390
}

private suspend fun loadSlingshotTokens(context: Context) =
    ChatGPTOAuth.requireStoredOrRefreshedTokens(
        context,
        context.getString(R.string.discovery_sign_in_slingshot),
    )

private sealed interface ManualServerProbeResult {
    data class Codex(val port: Int) : ManualServerProbeResult
    data class Ssh(val port: Int) : ManualServerProbeResult
    data object None : ManualServerProbeResult
}

private suspend fun probeManualServer(
    host: String,
    preferredCodexPort: Int?,
    timeoutMillis: Long,
): ManualServerProbeResult = withContext(Dispatchers.IO) {
    val codexPorts = orderedCodexPorts(preferredCodexPort)
    val sshPorts = orderedSshPorts()
    val deadline = System.currentTimeMillis() + maxOf(timeoutMillis, 500L)

    while (System.currentTimeMillis() < deadline) {
        for (port in codexPorts) {
            if (isPortOpen(host, port, 700)) {
                return@withContext ManualServerProbeResult.Codex(port)
            }
        }

        for (port in sshPorts) {
            if (isPortOpen(host, port, 700)) {
                return@withContext ManualServerProbeResult.Ssh(port)
            }
        }

        delay(350)
    }

    ManualServerProbeResult.None
}

private fun orderedCodexPorts(preferred: Int?): List<Int> = buildList {
    preferred?.let(::add)
    addAll(listOf(8390, 9234, 4222))
}.filter { it in 1..65535 }.distinct()

private fun orderedSshPorts(): List<Int> = listOf(22)

private fun isPortOpen(host: String, port: Int, timeoutMillis: Int): Boolean =
    runCatching {
        Socket().use { socket ->
            socket.connect(InetSocketAddress(host, port), timeoutMillis)
            true
        }
    }.getOrDefault(false)
