package com.litter.android.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.litter.android.state.isConnected
import com.sigkitten.litter.android.R
import com.litter.android.ui.LitterTheme
import com.litter.android.ui.LocalAppModel
import com.litter.android.ui.common.runtimeLabel
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.CancellationException
import org.json.JSONTokener
import org.json.JSONObject
import uniffi.codex_mobile_client.RuntimeSettingDescriptor
import uniffi.codex_mobile_client.RuntimeSettingValueKind

private data class HarnessTarget(val serverId: String, val runtime: String)
private data class HarnessServer(val id: String, val name: String, val runtimes: List<String>)

@Composable
fun HarnessSettingsScreen(onBack: () -> Unit) {
    val appModel = LocalAppModel.current
    var target by remember { mutableStateOf<HarnessTarget?>(null) }
    val selected = target
    if (selected != null) {
        RuntimeSettingsScreen(selected, onBack = { target = null })
        return
    }
    // Streaming conversation snapshots must not invalidate the settings UI.
    // Only connected-server/runtime changes reach Compose, and collection stops
    // altogether while one runtime's editor is open.
    val servers by remember(appModel) {
        appModel.snapshot.map { snapshot ->
            snapshot?.servers.orEmpty().filter { it.isConnected }.map { server ->
                HarnessServer(server.serverId, server.displayName,
                    server.agentRuntimes.filter { it.available }.map { it.kind }.distinct().sorted())
            }.sortedBy { it.id }
        }.distinctUntilChanged()
    }.collectAsState(initial = emptyList())
    LazyColumn(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            TextButton(onClick = onBack) { Text(stringResource(R.string.common_back), color = LitterTheme.accent) }
            Text(stringResource(R.string.settings_harnesses), color = LitterTheme.textPrimary)
        }
        if (servers.isEmpty()) {
            item { Text(stringResource(R.string.settings_harnesses_empty), color = LitterTheme.textSecondary) }
        }
        servers.forEach { server ->
            item(key = server.id) { Text(server.name, color = LitterTheme.textSecondary) }
            items(server.runtimes, key = { "${server.id}/$it" }) { runtime ->
                Text(runtime.runtimeLabel, color = LitterTheme.textPrimary,
                    modifier = Modifier.fillMaxWidth().background(LitterTheme.surface)
                        .clickable { target = HarnessTarget(server.id, runtime) }.padding(16.dp))
            }
        }
    }
}

@Composable
private fun RuntimeSettingsScreen(target: HarnessTarget, onBack: () -> Unit) {
    val appModel = LocalAppModel.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var settings by remember(target) { mutableStateOf<List<RuntimeSettingDescriptor>>(emptyList()) }
    var loading by remember(target) { mutableStateOf(false) }
    var error by remember(target) { mutableStateOf<String?>(null) }
    var search by remember(target) { mutableStateOf("") }
    var editing by remember(target) { mutableStateOf<RuntimeSettingDescriptor?>(null) }
    suspend fun refresh() {
        if (loading) return
        loading = true
        try {
            settings = appModel.client.runtimeSettings(target.serverId, target.runtime).settings
            error = null
        } catch (failure: CancellationException) { throw failure }
        catch (failure: Exception) { error = failure.message ?: context.getString(R.string.settings_load_failed) }
        finally { loading = false }
    }
    LaunchedEffect(target) { refresh() }
    LazyColumn(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Row {
                TextButton(onClick = onBack) { Text(stringResource(R.string.common_back), color = LitterTheme.accent) }
                TextButton(onClick = { scope.launch { refresh() } }, enabled = !loading) { Text(stringResource(R.string.common_refresh), color = LitterTheme.accent) }
            }
            Text(target.runtime.runtimeLabel, color = LitterTheme.textPrimary)
            Text(stringResource(R.string.settings_harness_native_help), color = LitterTheme.textSecondary)
            OutlinedTextField(value = search, onValueChange = { search = it }, label = { Text(stringResource(R.string.settings_find_setting)) }, modifier = Modifier.fillMaxWidth())
        }
        if (loading) item { CircularProgressIndicator(color = LitterTheme.accent) }
        error?.let { item { Text(it, color = LitterTheme.danger) } }
        if (!loading && error == null && settings.isEmpty()) {
            item { Text(stringResource(R.string.settings_harness_no_settings), color = LitterTheme.textSecondary) }
        }
        items(settings.filter { search.isBlank() || it.key.contains(search, true) || it.label.contains(search, true) }, key = { it.key }) { setting ->
            Column(Modifier.fillMaxWidth().background(LitterTheme.surface).clickable(enabled = !loading) { editing = setting }.padding(12.dp)) {
                Text(setting.label, color = LitterTheme.textPrimary)
                Text(setting.valueJson, color = LitterTheme.textSecondary, maxLines = 2)
                setting.readOnlyReason?.let { Text(it, color = LitterTheme.textMuted) }
            }
        }
    }
    editing?.let { setting ->
        RuntimeSettingEditor(setting, onDismiss = { editing = null }) { value ->
            settings = appModel.client.setRuntimeSetting(target.serverId, target.runtime, setting.key, value).settings
            error = null
            if (setting.key == "\$native" || setting.key.contains("model", ignoreCase = true) || setting.key.contains("provider", ignoreCase = true)) {
                scope.launch { appModel.loadAvailableModelsIfNeeded(target.serverId, force = true) }
            }
        }
    }
}

@Composable
internal fun RuntimeSettingEditor(setting: RuntimeSettingDescriptor, onDismiss: () -> Unit, save: suspend (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var value by remember(setting) {
        mutableStateOf(if (setting.valueKind == RuntimeSettingValueKind.STRING && setting.valueJson == "null") ""
        else if (setting.valueKind == RuntimeSettingValueKind.STRING) {
            runCatching { JSONTokener(setting.valueJson).nextValue() as String }.getOrDefault(setting.valueJson)
        } else setting.valueJson)
    }
    var saving by remember { mutableStateOf(false) }
    var edited by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    AlertDialog(
        onDismissRequest = { if (!saving) onDismiss() },
        title = { Text(setting.label) },
        text = {
            LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                item {
                    if (setting.valueJson == "null" && !edited) Text(stringResource(R.string.settings_unset))
                    if (setting.valueKind == RuntimeSettingValueKind.BOOLEAN && setting.valueJson == "null") {
                        if (edited) {
                            Text(
                                stringResource(
                                    if (value == "true") R.string.settings_selected_enabled else R.string.settings_selected_disabled,
                                ),
                            )
                        }
                        Row {
                            listOf(R.string.settings_enabled to "true", R.string.settings_disabled to "false").forEach { (label, json) ->
                                TextButton(onClick = { value = json; edited = true }, enabled = setting.writable && !saving) {
                                    Text(stringResource(label))
                                }
                            }
                        }
                    } else if (setting.valueKind == RuntimeSettingValueKind.BOOLEAN) {
                        Row {
                            Text(stringResource(R.string.settings_enabled), modifier = Modifier.weight(1f))
                            Switch(checked = value == "true", onCheckedChange = { value = it.toString(); edited = true }, enabled = setting.writable && !saving)
                        }
                    } else {
                        OutlinedTextField(value = value, onValueChange = { value = it; edited = true },
                            label = {
                                Text(
                                    stringResource(
                                        if (setting.valueKind == RuntimeSettingValueKind.JSON) {
                                            R.string.settings_json_value
                                        } else {
                                            R.string.settings_value
                                        },
                                    ),
                                )
                            },
                            enabled = setting.writable && !saving, minLines = 3, maxLines = 12)
                    }
                }
                items(setting.choices) { choice ->
                    TextButton(onClick = {
                        value = if (setting.valueKind == RuntimeSettingValueKind.STRING) choice else JSONObject.quote(choice)
                        edited = true
                    }, enabled = setting.writable && !saving) { Text(choice) }
                }
                item {
                    Text(setting.source)
                    Text(stringResource(R.string.settings_scope, setting.scope))
                    setting.readOnlyReason?.let { Text(it) }
                    error?.let { Text(it, color = LitterTheme.danger) }
                }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !saving) { Text(stringResource(R.string.common_cancel)) } },
        confirmButton = {
            TextButton(enabled = setting.writable && !saving && edited, onClick = {
                saving = true
                scope.launch {
                    try {
                        save(if (setting.valueKind == RuntimeSettingValueKind.STRING) JSONObject.quote(value) else value)
                        onDismiss()
                    } catch (failure: CancellationException) { throw failure }
                    catch (failure: Exception) { error = failure.message }
                    finally { saving = false }
                }
            }) { Text(stringResource(if (saving) R.string.settings_saving else R.string.common_save)) }
        },
    )
}
