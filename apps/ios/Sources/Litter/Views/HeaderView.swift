import SafariServices
import SwiftUI

struct ConversationToolbarControls: View {
    enum Control {
        case reload
        case info
    }

    @Environment(AppState.self) private var appState
    @Environment(AppModel.self) private var appModel
    let thread: AppThreadSnapshot
    let control: Control
    var onInfo: (() -> Void)?
    var server: AppServerSnapshot?
    /// Inside a navigation bar the system supplies sizing and chrome.
    var inToolbar: Bool = false
    @State private var isReloading = false
    @State private var remoteAuthSession: RemoteAuthSession?

    @ViewBuilder
    private var control_: some View {
        switch control {
        case .reload:
            reloadButton
        case .info:
            infoButton
        }
    }

    var body: some View {
        Group {
            if inToolbar {
                control_
            } else {
                control_
                    .frame(width: 40, height: 40)
                    .contentShape(Circle())
                    .buttonStyle(.plain)
                    .modifier(GlassCircleModifier())
                    .hoverEffect(.highlight)
            }
        }
        .sheet(item: $remoteAuthSession) { session in
            InAppSafariView(url: session.url)
                .ignoresSafeArea()
        }
        .onChange(of: server?.account != nil) { _, isLoggedIn in
            if isLoggedIn {
                remoteAuthSession = nil
            }
        }
    }

    private var reloadButton: some View {
        Button {
            Task {
                isReloading = true
                defer { isReloading = false }
                if await handleRemoteLoginIfNeeded() {
                    return
                }
                if server?.requiresOpenaiAuth == true, server?.account == nil {
                    appState.showSettings = true
                } else {
                    do {
                        let nextKey = try await appModel.refreshThreadIncludingTurns(key: thread.key)
                        appModel.store.setActiveThread(
                            key: nextKey
                        )
                    } catch {
                        // `AppModel` records the failure; keep the toolbar interaction quiet.
                    }
                }
            }
        } label: {
            reloadButtonLabel
        }
        .accessibilityIdentifier("header.reloadButton")
        .disabled(isReloading || server?.isConnected != true)
    }

    @ViewBuilder
    private var reloadButtonLabel: some View {
        if isReloading {
            ProgressView()
                .scaleEffect(0.7)
                .tint(LitterTheme.accent)
        } else {
            Image(systemName: "arrow.clockwise")
                .font(.system(size: 15, weight: .medium))
                .foregroundColor(server?.isConnected == true ? LitterTheme.textPrimary : LitterTheme.textMuted)
        }
    }

    private var infoButton: some View {
        Button {
            onInfo?()
        } label: {
            Image(systemName: "info.circle")
                .font(.system(size: 15, weight: .medium))
                .foregroundColor(LitterTheme.textPrimary)
        }
        .accessibilityIdentifier("header.infoButton")
    }

    private func handleRemoteLoginIfNeeded() async -> Bool {
        guard let server, !server.isLocal else {
            return false
        }
        guard server.requiresOpenaiAuth, server.account == nil else {
            return false
        }
        do {
            let authURL = try await appModel.client.startRemoteSshOauthLogin(
                serverId: server.serverId
            )
            if let url = URL(string: authURL) {
                await MainActor.run {
                    remoteAuthSession = RemoteAuthSession(url: url)
                }
            }
        } catch {}
        return true
    }
}

private struct RemoteAuthSession: Identifiable {
    let id = UUID()
    let url: URL
}

func modelMatchesSelection(
    _ model: ModelInfo,
    _ selection: String,
    runtime: AgentRuntimeKind? = nil
) -> Bool {
    let trimmed = selection.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return false }
    if let runtime, model.agentRuntimeKind != runtime { return false }
    return model.id == trimmed || model.model == trimmed
}

/// Chip and header label for a catalog entry: the mode name for mode
/// entries (Amp `high`), otherwise the model's display name.
func modelPickerDisplayName(_ model: ModelInfo) -> String {
    if model.isModeEntry, !model.pickerName.isEmpty { return model.pickerName }
    return model.displayName.isEmpty ? model.id : model.displayName
}

/// Model picker opened from the home composer chip or a thread's options
/// sheet. Adds plan-mode and permission toggles to the shared picker.
struct InlineModelSelectorView: View {
    let models: [ModelInfo]
    var catalogLoaded = false
    var catalogError: String?
    var onRetryModels: () -> Void = {}
    @Binding var selectedModel: String
    @Binding var selectedAgentRuntimeKind: AgentRuntimeKind?
    @Binding var reasoningEffort: String
    /// `nil` indicates the view is being used before a thread exists (home
    /// composer). In that case, plan-mode selection is stored as a pending
    /// app-state preference that the caller applies after `startThread`.
    var threadKey: ThreadKey?
    var collaborationMode: AppModeKind = .default
    var effectiveApprovalPolicy: AppAskForApproval?
    var effectiveSandboxPolicy: AppSandboxPolicy?
    var isReasoningEffortLocked = false
    var onDismiss: () -> Void

    var body: some View {
        let derived = self.derived
        let currentModel = self.currentModel(in: derived.visibleModels)
        let visibleModels = searchIndexCache
            .searchIndex(for: derived.runtimeScopedModels)
            .results(matching: modelSearchQuery)
        let selectedModelIsAmp: Bool = {
            guard let model = currentModel else { return false }
            return visibleModeNames(for: model.agentRuntimeKind) != nil
        }()
        let effectiveReasoningEfforts = isReasoningEffortLocked ? [] : (currentModel?.supportedReasoningEfforts ?? [])

        VStack(spacing: 0) {
            modelSearchField
            runtimeFilterRow(derived)

            ScrollView {
                LazyVStack(spacing: 0) {
                    modelCatalogNotice(
                        loaded: catalogLoaded,
                        error: catalogError,
                        hasModels: !derived.visibleModels.isEmpty,
                        horizontalPadding: 16,
                        onRetry: onRetryModels
                    )

                    if !derived.visibleModels.isEmpty && visibleModels.isEmpty {
                        Text("No matching models")
                            .litterFont(.caption)
                            .foregroundColor(LitterTheme.textSecondary)
                            .frame(maxWidth: .infinity, alignment: .center)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 24)
                    }

                    ForEach(providerGroupCache.groups(for: visibleModels)) { group in
                        Text(group.name.uppercased())
                            .litterFont(.caption2, weight: .semibold)
                            .foregroundColor(LitterTheme.textMuted)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 16)
                            .padding(.top, 12)
                            .padding(.bottom, 4)

                        ForEach(group.models, id: \.runtimeScopedID) { model in
                            Button {
                                selectedModel = model.id
                                selectedAgentRuntimeKind = model.agentRuntimeKind
                                if isReasoningEffortLocked && visibleModeNames(for: model.agentRuntimeKind) != nil {
                                    reasoningEffort = ""
                                } else {
                                    reasoningEffort = defaultReasoningEffortSelection(for: model)
                                }
                                if threadKey != nil { onDismiss() }
                            } label: {
                                HStack {
                                    ModelRuntimeIcon(kind: model.agentRuntimeKind)

                                    VStack(alignment: .leading, spacing: 2) {
                                        HStack(spacing: 6) {
                                            Text(modelNameWithinProvider(model))
                                                .litterFont(.footnote)
                                                .foregroundColor(LitterTheme.textPrimary)
                                            if model.isDefault {
                                                Text("default")
                                                    .litterFont(.caption2, weight: .medium)
                                                    .foregroundColor(LitterTheme.accent)
                                                    .padding(.horizontal, 6)
                                                    .padding(.vertical, 1)
                                                    .background(LitterTheme.accent.opacity(0.15))
                                                    .clipShape(Capsule())
                                            }
                                        }
                                        Text(model.description)
                                            .litterFont(.caption2)
                                            .foregroundColor(LitterTheme.textSecondary)
                                    }
                                    Spacer()
                                    if modelMatchesSelection(
                                        model,
                                        selectedModel,
                                        runtime: selectedAgentRuntimeKind
                                    ) {
                                        Image(systemName: "checkmark")
                                            .litterFont(size: 12, weight: .medium)
                                            .foregroundColor(LitterTheme.accent)
                                    }
                                }
                                .padding(.horizontal, 16)
                                .padding(.vertical, 8)
                            }
                            if model.id != group.models.last?.id {
                                Divider().background(LitterTheme.separator).padding(.leading, 16)
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)

            if isReasoningEffortLocked && selectedModelIsAmp {
                Divider().background(LitterTheme.separator).padding(.horizontal, 12)

                Text("Reasoning effort is locked after the first message.")
                    .litterFont(.caption2)
                    .foregroundColor(LitterTheme.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
            } else if !effectiveReasoningEfforts.isEmpty {
                Divider().background(LitterTheme.separator).padding(.horizontal, 12)

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(effectiveReasoningEfforts) { effort in
                            Button {
                                reasoningEffort = effort.reasoningEffort.wireValue
                                onDismiss()
                            } label: {
                                Text(effort.reasoningEffort.wireValue)
                                    .litterFont(.caption2, weight: .medium)
                                    .foregroundColor(effort.reasoningEffort.wireValue == reasoningEffort ? LitterTheme.textOnAccent : LitterTheme.textPrimary)
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 5)
                                    .background(effort.reasoningEffort.wireValue == reasoningEffort ? LitterTheme.accent : LitterTheme.surfaceLight)
                                    .clipShape(Capsule())
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                }
            }

            Divider().background(LitterTheme.separator).padding(.horizontal, 12)

            HStack(spacing: 6) {
                Button {
                    let current = effectiveCollaborationMode
                    let next: AppModeKind = current == .plan ? .default : .plan
                    if let threadKey {
                        Task {
                            try? await appModel.store.setThreadCollaborationMode(
                                key: threadKey, mode: next
                            )
                        }
                    } else {
                        appState.pendingCollaborationMode = next
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.text")
                            .litterFont(size: 9, weight: .semibold)
                        Text("Plan")
                            .litterFont(.caption2, weight: .medium)
                    }
                    .foregroundColor(effectiveCollaborationMode == .plan ? .black : LitterTheme.textPrimary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(effectiveCollaborationMode == .plan ? LitterTheme.accent : LitterTheme.surfaceLight)
                    .clipShape(Capsule())
                }

                Button {
                    fastMode.toggle()
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "bolt.fill")
                            .litterFont(size: 9, weight: .semibold)
                        Text("Fast")
                            .litterFont(.caption2, weight: .medium)
                    }
                    .foregroundColor(fastMode ? LitterTheme.textOnAccent : LitterTheme.textPrimary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(fastMode ? LitterTheme.warning : LitterTheme.surfaceLight)
                    .clipShape(Capsule())
                }

                if selectedRuntimeSupportsPermissionOverrides(currentModel) {
                    Button {
                        if isFullAccess {
                            appState.setPermissions(approvalPolicy: "on-request", sandboxMode: "workspace-write", for: threadKey)
                        } else {
                            appState.setPermissions(approvalPolicy: "never", sandboxMode: "danger-full-access", for: threadKey)
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: isFullAccess ? "lock.open.fill" : "lock.fill")
                                .litterFont(size: 9, weight: .semibold)
                            Text(AppLanguage.localizedString(isFullAccess ? "Full Access" : "Supervised"))
                                .litterFont(.caption2, weight: .medium)
                        }
                        .foregroundColor(isFullAccess ? LitterTheme.textOnAccent : LitterTheme.textPrimary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(isFullAccess ? LitterTheme.danger : LitterTheme.surfaceLight)
                        .clipShape(Capsule())
                    }
                }

                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .padding(.vertical, 4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(showsBackground ? LitterTheme.surface : Color.clear)
        // The search index is no longer (re)built here: `searchIndexCache`
        // builds it lazily from the scoped model list, so presentation
        // constructs it exactly once instead of once speculatively from
        // `body` plus once from `.onAppear`.
        .onAppear {
            synchronizeRuntimeFilter()
        }
        .onChange(of: models) { _, _ in
            synchronizeRuntimeFilter()
        }
        .onChange(of: selectedAgentRuntimeKind) { _, _ in
            synchronizeRuntimeFilter()
        }
    }

    private var modelSearchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(LitterTheme.textMuted)
            TextField("Search models", text: $modelSearchQuery)
                .litterFont(.caption)
                .foregroundStyle(LitterTheme.textPrimary)
                .tint(LitterTheme.accent)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
            if !modelSearchQuery.isEmpty {
                Button { modelSearchQuery = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(LitterTheme.textMuted)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }

    @ViewBuilder
    private func runtimeFilterRow(_ derived: ModelSelectorDerivation) -> some View {
        if derived.runtimeBuckets.count > 1 {
            RuntimeFilterRow(
                buckets: derived.runtimeBuckets,
                totalCount: derived.visibleModels.count,
                selectedRuntime: derived.activeRuntimeFilter,
                onSelect: { selectedRuntimeFilter = $0 }
            )
            .padding(.bottom, 6)
        }
    }

    private func synchronizeRuntimeFilter() {
        let derived = self.derived
        if !initializedRuntimeFilter {
            let initial = selectedAgentRuntimeKind
                ?? self.currentModel(in: derived.visibleModels)?.agentRuntimeKind
            if let initial, derived.runtimeBuckets.contains(where: { $0.kind == initial }) {
                selectedRuntimeFilter = initial
            }
            initializedRuntimeFilter = true
            return
        }
        if let selectedRuntimeFilter,
           !derived.runtimeBuckets.contains(where: { $0.kind == selectedRuntimeFilter }) {
            self.selectedRuntimeFilter = nil
        }
    }
}

private struct InAppSafariView: UIViewControllerRepresentable {
    let url: URL

    func makeUIViewController(context: Context) -> SFSafariViewController {
        let controller = SFSafariViewController(url: url)
        controller.dismissButtonStyle = .close
        return controller
    }

    func updateUIViewController(_ uiViewController: SFSafariViewController, context: Context) {}
}

/// Model picker opened from the conversation composer.
struct ModelSelectorSheet: View {
    let models: [ModelInfo]
    var catalogLoaded = false
    var catalogError: String?
    var onRetryModels: () -> Void = {}
    @Binding var selectedModel: String
    @Binding var selectedAgentRuntimeKind: AgentRuntimeKind?
    @Binding var reasoningEffort: String
    var isReasoningEffortLocked = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ModelPickerView(
            models: models,
            catalogLoaded: catalogLoaded,
            catalogError: catalogError,
            onRetryModels: onRetryModels,
            selectedModel: $selectedModel,
            selectedAgentRuntimeKind: $selectedAgentRuntimeKind,
            reasoningEffort: $reasoningEffort,
            isReasoningEffortLocked: isReasoningEffortLocked,
            onDone: { dismiss() }
        )
    }
}
