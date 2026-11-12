import SwiftUI
import UIKit

struct ConversationComposerEntryRowView: View {
    @Binding var showAttachMenu: Bool
    @Binding var inputText: String
    @Binding var isComposerFocused: Bool
    @Binding var composerSelectionRange: NSRange
    let voiceManager: VoiceTranscriptionManager
    let isTurnActive: Bool
    let hasAttachment: Bool
    let allowsVoiceInput: Bool
    let modelLabel: String?
    let reasoningLabel: String?
    let collaborationMode: AppModeKind
    let showModeChip: Bool
    let showsPermissionMenu: Bool
    let permissionThreadKey: ThreadKey?
    let effectiveApprovalPolicy: AppAskForApproval?
    let effectiveSandboxPolicy: AppSandboxPolicy?
    let showsCollaborationMode: Bool
    let onPasteImage: (UIImage) -> Void
    let onOpenModelPicker: () -> Void
    let onOpenModePicker: () -> Void
    let onSendText: () -> Void
    let onStopRecording: () -> Void
    let onStartRecording: () -> Void
    let onInterrupt: () -> Void

    private enum Metrics {
        static let controlSize: CGFloat = 40
        static let inputCornerRadius: CGFloat = 26
        static let trailingControlSize: CGFloat = 40
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 6
    }

    init(
        showAttachMenu: Binding<Bool>,
        inputText: Binding<String>,
        isComposerFocused: Binding<Bool>,
        composerSelectionRange: Binding<NSRange> = .constant(NSRange(location: 0, length: 0)),
        voiceManager: VoiceTranscriptionManager,
        isTurnActive: Bool,
        hasAttachment: Bool,
        allowsVoiceInput: Bool = true,
        modelLabel: String? = nil,
        reasoningLabel: String? = nil,
        collaborationMode: AppModeKind = .`default`,
        showModeChip: Bool = false,
        showsPermissionMenu: Bool = false,
        permissionThreadKey: ThreadKey? = nil,
        effectiveApprovalPolicy: AppAskForApproval? = nil,
        effectiveSandboxPolicy: AppSandboxPolicy? = nil,
        showsCollaborationMode: Bool = false,
        onPasteImage: @escaping (UIImage) -> Void,
        onOpenModelPicker: @escaping () -> Void = {},
        onOpenModePicker: @escaping () -> Void = {},
        onSendText: @escaping () -> Void,
        onStopRecording: @escaping () -> Void,
        onStartRecording: @escaping () -> Void,
        onInterrupt: @escaping () -> Void
    ) {
        _showAttachMenu = showAttachMenu
        _inputText = inputText
        _isComposerFocused = isComposerFocused
        _composerSelectionRange = composerSelectionRange
        self.voiceManager = voiceManager
        self.isTurnActive = isTurnActive
        self.hasAttachment = hasAttachment
        self.allowsVoiceInput = allowsVoiceInput
        self.modelLabel = modelLabel
        self.reasoningLabel = reasoningLabel
        self.collaborationMode = collaborationMode
        self.showModeChip = showModeChip
        self.showsPermissionMenu = showsPermissionMenu
        self.permissionThreadKey = permissionThreadKey
        self.effectiveApprovalPolicy = effectiveApprovalPolicy
        self.effectiveSandboxPolicy = effectiveSandboxPolicy
        self.showsCollaborationMode = showsCollaborationMode
        self.onPasteImage = onPasteImage
        self.onOpenModelPicker = onOpenModelPicker
        self.onOpenModePicker = onOpenModePicker
        self.onSendText = onSendText
        self.onStopRecording = onStopRecording
        self.onStartRecording = onStartRecording
        self.onInterrupt = onInterrupt
    }

    @State private var showExpanded: Bool = false

    /// Equivalent to `!inputText.trimmingCharacters(in: .whitespaces).isEmpty`
    /// without allocating a trimmed copy of the whole draft on every body
    /// pass. `.whitespaces` does not include newlines, so a newline still
    /// counts as content — hence the explicit `isNewline` arm.
    private var hasText: Bool {
        inputText.contains(where: { !$0.isWhitespace || $0.isNewline })
    }

    private var canSend: Bool {
        hasText || hasAttachment
    }

    /// Show the expand affordance once the composer is multi-line or starts to
    /// wrap, matching ChatGPT's behaviour. Short prompts stay clutter-free.
    private var shouldShowExpand: Bool {
        !voiceManager.isRecording
            && !voiceManager.isTranscribing
            && isMultilineOrLong
    }

    /// Same predicate as `inputText.contains("\n") || inputText.count > 60`,
    /// but one early-exiting pass bounded at 61 characters instead of two full
    /// walks of the draft on every body pass.
    private var isMultilineOrLong: Bool {
        var count = 0
        for character in inputText {
            if character == "\n" { return true }
            count += 1
            if count > 60 { return true }
        }
        return false
    }

    var body: some View {
        VStack(spacing: 0) {
            textEditor
            actionRow
        }
        .modifier(GlassRoundedRectModifier(cornerRadius: Metrics.inputCornerRadius))
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.spring(response: 0.3, dampingFraction: 0.86), value: isTurnActive)
        .animation(.spring(response: 0.3, dampingFraction: 0.86), value: canSend)
        .padding(.horizontal, Metrics.horizontalPadding)
        .padding(.top, Metrics.verticalPadding)
        .padding(.bottom, Metrics.verticalPadding)
        .fullScreenCover(isPresented: $showExpanded) {
            ConversationComposerExpandedView(
                inputText: $inputText,
                isPresented: $showExpanded,
                onPasteImage: onPasteImage,
                onSend: onSendText,
                hasAttachment: hasAttachment
            )
        }
    }

    private var textEditor: some View {
        ZStack(alignment: .topLeading) {
            ConversationComposerTextView(
                text: $inputText,
                isFocused: $isComposerFocused,
                selectedRange: $composerSelectionRange,
                onPasteImage: onPasteImage,
                onHardwareSubmit: {
                    if canSend { onSendText() }
                },
                horizontalInset: 10,
                verticalInset: 10
            )

            if inputText.isEmpty {
                Text("Ask anything")
                    .font(LitterFont.styled(size: 17))
                    .foregroundColor(LitterTheme.textMuted)
                    .padding(.leading, 10)
                    .padding(.top, 10)
                    .allowsHitTesting(false)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading)
        .overlay(alignment: .topTrailing) {
            if shouldShowExpand {
                Button {
                    showExpanded = true
                } label: {
                    Image(systemName: "arrow.up.left.and.arrow.down.right")
                        .font(LitterFont.styled(size: 12, weight: .semibold))
                        .foregroundColor(LitterTheme.textSecondary)
                        .padding(8)
                }
                .buttonStyle(.plain)
                .hoverEffect(.highlight)
                .accessibilityLabel("Expand composer")
                .transition(.opacity.combined(with: .scale))
            }
        }
        .animation(.easeInOut(duration: 0.15), value: shouldShowExpand)
    }

    private var actionRow: some View {
        HStack(spacing: 6) {
            if !voiceManager.isRecording && !voiceManager.isTranscribing {
                composerCircleButton(systemName: "plus", label: "Attach") {
                    showAttachMenu = true
                }

                if showsPermissionMenu {
                    ConversationComposerPermissionMenu(
                        threadKey: permissionThreadKey,
                        effectiveApprovalPolicy: effectiveApprovalPolicy,
                        effectiveSandboxPolicy: effectiveSandboxPolicy,
                        showsCollaborationMode: showsCollaborationMode,
                        onOpenCollaborationMode: onOpenModePicker
                    )
                }
            }

            if showModeChip, !showsPermissionMenu {
                ConversationComposerModeChip(mode: collaborationMode, onTap: onOpenModePicker)
            }

            Spacer(minLength: 8)

            if let modelLabel {
                Button(action: onOpenModelPicker) {
                    HStack(spacing: 3) {
                        Text(modelLabel)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        if let reasoningLabel, reasoningLabel != "default" {
                            Text(reasoningLabel)
                                .lineLimit(1)
                        }
                        Image(systemName: "chevron.down")
                            .font(.system(size: 9, weight: .medium))
                    }
                    .font(LitterFont.styled(size: 13, weight: .medium))
                    .foregroundColor(LitterTheme.textPrimary)
                    .padding(.vertical, 6)
                }
                .buttonStyle(.plain)
                .hoverEffect(.highlight)
                .accessibilityIdentifier("conversation.modelPickerButton")
                .accessibilityLabel("Choose model")
                .frame(minWidth: 0, alignment: .trailing)
                .layoutPriority(1)
            }

            if voiceManager.isRecording {
                AudioWaveformView(level: voiceManager.audioLevel)
                    .frame(width: 42, height: 20)
            }
            trailingControl
                .fixedSize()
                .layoutPriority(3)
        }
        .padding(.horizontal, 6)
        .padding(.bottom, 6)
        .frame(maxWidth: .infinity, minHeight: 42)
    }

    /// Idle empty state keeps the mic and the up-arrow together. The arrow
    /// turns accent only when there is something to send, and a running turn
    /// replaces it with stop. Recording and transcription still take the row.
    @ViewBuilder
    private var trailingControl: some View {
        if voiceManager.isRecording {
            composerCircleButton(systemName: "stop.fill", label: "Stop recording", tint: .black, fill: LitterTheme.accentStrong) {
                onStopRecording()
            }
        } else if voiceManager.isTranscribing {
            ProgressView()
                .tint(LitterTheme.accent)
                .frame(width: Metrics.trailingControlSize, height: Metrics.trailingControlSize)
        } else {
            HStack(spacing: 6) {
                if allowsVoiceInput, !canSend, !isTurnActive {
                    composerCircleButton(systemName: "mic", label: "Dictate") {
                        onStartRecording()
                    }
                }
                if canSend {
                    composerCircleButton(systemName: "arrow.up", label: "Send", tint: .black, fill: LitterTheme.accent) {
                        onSendText()
                    }
                } else if isTurnActive {
                    composerCircleButton(systemName: "stop.fill", label: "Cancel response", tint: LitterTheme.surface, fill: LitterTheme.textPrimary) {
                        onInterrupt()
                    }
                } else {
                    composerCircleButton(
                        systemName: "arrow.up",
                        label: "Send",
                        tint: LitterTheme.textMuted
                    ) {}
                    .disabled(true)
                }
            }
        }
    }

    private func composerCircleButton(
        systemName: String,
        label: String,
        tint: Color = LitterTheme.textPrimary,
        fill: Color = LitterTheme.surfaceLight.opacity(0.72),
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(LitterFont.styled(size: systemName == "plus" ? 20 : 16, weight: .semibold))
                .foregroundColor(tint)
                .frame(width: Metrics.controlSize, height: Metrics.controlSize)
                .background(Circle().fill(fill))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .accessibilityLabel(Text(LocalizedStringKey(label)))
        .accessibilityIdentifier("conversation.\(label.lowercased().replacingOccurrences(of: " ", with: "-"))Button")
    }
}

/// Shield + current preset on the composer row. The menu is the upstream
/// permission control: read only, auto, full access, then collaboration mode.
///
/// The button itself is the menu source. On iOS 26 the source hides into the
/// menu, and the selected row slides back into it from the lower right. A
/// separate SwiftUI label on top of a clear button never joins that morph, so
/// the menu flashes and the text snaps.
private struct ConversationComposerPermissionMenu: View {
    @Environment(AppState.self) private var appState
    let threadKey: ThreadKey?
    let effectiveApprovalPolicy: AppAskForApproval?
    let effectiveSandboxPolicy: AppSandboxPolicy?
    let showsCollaborationMode: Bool
    let onOpenCollaborationMode: () -> Void

    private var preset: AppThreadPermissionPreset {
        let approval = appState.launchApprovalPolicy(for: threadKey) ?? effectiveApprovalPolicy
        let sandbox = appState.turnSandboxPolicy(for: threadKey) ?? effectiveSandboxPolicy
        return threadPermissionPreset(approvalPolicy: approval, sandboxPolicy: sandbox)
    }

    private var selectedChoice: SessionPermissionChoice? {
        SessionPermissionChoice.matching(preset)
    }

    private var labelKey: String {
        if let selectedChoice {
            return selectedChoice.menuTitleKey
        }
        return preset == .custom ? "Custom" : "Auto"
    }

    private var labelColor: Color {
        selectedChoice == .fullAccess ? LitterTheme.warning : LitterTheme.textSecondary
    }

    var body: some View {
        ConversationComposerPermissionMenuButton(
            accessibilityLabel: AppLanguage.localizedString("Permissions"),
            title: AppLanguage.localizedString(labelKey),
            symbolName: permissionSymbolName,
            foreground: UIColor(labelColor),
            items: SessionPermissionChoice.allCases.map { choice in
                let title = AppLanguage.localizedString(choice.menuTitleKey)
                return ConversationComposerPermissionMenuButton.Item(
                    title: title,
                    subtitle: AppLanguage.localizedString(choice.menuDetailKey),
                    symbolName: Self.symbolName(for: choice),
                    foreground: UIColor(choice == .fullAccess ? LitterTheme.warning : LitterTheme.textSecondary),
                    selected: choice == selectedChoice,
                    action: {
                        appState.setPermissions(
                            approvalPolicy: choice.approvalPolicy,
                            sandboxMode: choice.sandboxMode,
                            for: threadKey
                        )
                    }
                )
            },
            collaborationTitle: showsCollaborationMode
                ? AppLanguage.localizedString("Collaboration mode...")
                : nil,
            onOpenCollaborationMode: onOpenCollaborationMode
        )
        .fixedSize(horizontal: true, vertical: false)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(LocalizedStringKey(labelKey)))
        .accessibilityIdentifier("conversation.permissionMenu")
        .accessibilityAddTraits(.isButton)
    }

    /// Read only is a shield with a lock. Auto is an outline shield. Full access
    /// is an outline shield with an exclamation mark, tinted warning.
    private var permissionSymbolName: String {
        Self.symbolName(for: selectedChoice)
    }

    private static func symbolName(for choice: SessionPermissionChoice?) -> String {
        switch choice {
        case .readOnly:
            return "lock.shield"
        case .fullAccess:
            return "exclamationmark.shield"
        case .auto, .none:
            return "shield"
        }
    }
}

/// Pop-up button so choosing a preset plays the system selection morph.
/// Menu rows stay title + subtitle + checkmark; the shield lives on the button.
private struct ConversationComposerPermissionMenuButton: UIViewRepresentable {
    struct Item {
        let title: String
        let subtitle: String
        let symbolName: String
        let foreground: UIColor
        let selected: Bool
        let action: () -> Void
    }

    let accessibilityLabel: String
    let title: String
    let symbolName: String
    let foreground: UIColor
    let items: [Item]
    let collaborationTitle: String?
    let onOpenCollaborationMode: () -> Void

    func makeUIView(context: Context) -> PermissionMenuButton {
        let button = PermissionMenuButton(type: .custom)
        button.showsMenuAsPrimaryAction = true
        // Pop-up tracking is what slides the chosen row back into this button.
        // Collaboration is kept off that path in the action handler.
        button.changesSelectionAsPrimaryAction = true
        button.preferredMenuElementOrder = .fixed
        button.contentHorizontalAlignment = .leading
        button.contentVerticalAlignment = .center
        button.backgroundColor = .clear
        button.configurationUpdateHandler = { button in
            (button as? PermissionMenuButton)?.fixConfiguration()
        }
        return button
    }

    func updateUIView(_ button: PermissionMenuButton, context: Context) {
        button.accessibilityIdentifier = "conversation.permissionMenu"
        button.accessibilityLabel = accessibilityLabel
        button.onOpenCollaborationMode = onOpenCollaborationMode

        var styles: [String: PermissionMenuButton.ChoiceStyle] = [:]
        for item in items {
            styles[item.title] = PermissionMenuButton.ChoiceStyle(
                title: item.title,
                symbolName: item.symbolName,
                foreground: item.foreground
            )
        }
        button.choiceStyles = styles

        let appearance = PermissionMenuButton.Appearance(
            title: title,
            symbolName: symbolName,
            foreground: foreground,
            collaborationTitle: collaborationTitle
        )
        var children: [UIMenuElement] = [
            compactMenu(options: [.displayInline, .singleSelection], children: items.map { item in
                UIAction(
                    title: item.title,
                    subtitle: item.subtitle,
                    identifier: UIAction.Identifier(rawValue: "litter.permission.\(item.title)"),
                    state: item.selected ? .on : .off
                ) { [weak button] _ in
                    MainActor.assumeIsolated {
                        // The source label is hidden while the menu is up.
                        // Point it at the new preset now so the dismiss morph
                        // slides that option back in, instead of the old one.
                        button?.adopt(
                            title: item.title,
                            symbolName: item.symbolName,
                            foreground: item.foreground
                        )
                        item.action()
                    }
                }
            })
        ]
        if let collaborationTitle {
            children.append(
                compactMenu(options: .displayInline, children: [
                    UIAction(
                        title: collaborationTitle,
                        image: UIImage(systemName: "list.clipboard"),
                        identifier: UIAction.Identifier(rawValue: "litter.permission.collaboration")
                    ) { [weak button] action in
                        MainActor.assumeIsolated {
                            // This row is not a permission. Keep the button on
                            // the current preset and open the sheet after the
                            // menu has finished leaving.
                            action.state = .off
                            button?.keepCurrentPreset()
                            button?.wantsCollaboration = true
                        }
                    }
                ])
            )
        }
        let menu = compactMenu(children: children)
        let signature = menuSignature(appearance: appearance, items: items, collaborationTitle: collaborationTitle)
        button.sync(appearance: appearance, menu: menu, signature: signature)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: PermissionMenuButton, context: Context) -> CGSize? {
        let fitted = uiView.intrinsicContentSize
        let width = fitted.width > 1 ? fitted.width : 44
        return CGSize(width: ceil(width), height: 34)
    }

    /// Vertical list, not the automatic palette. A plain large button otherwise
    /// presents the iOS 26 menu at the large control size, which is much taller
    /// than the upstream composer menu. Subtitles stay on one line.
    private func compactMenu(options: UIMenu.Options = [], children: [UIMenuElement]) -> UIMenu {
        var menu = UIMenu(options: options, preferredElementSize: .large, children: children)
        let preferences = UIMenuDisplayPreferences()
        preferences.maximumNumberOfTitleLines = 2
        menu.displayPreferences = preferences
        return menu
    }

    private func menuSignature(
        appearance: PermissionMenuButton.Appearance,
        items: [Item],
        collaborationTitle: String?
    ) -> String {
        let rows = items.map { "\($0.title)|\($0.subtitle)|\($0.selected)" }.joined(separator: ";")
        return "\(appearance.title)|\(rows)|\(collaborationTitle ?? "")"
    }
}

/// Visible menu source. Selecting a preset updates this button while the menu
/// is still held; the menu object itself is replaced only after dismiss.
private final class PermissionMenuButton: UIButton {
    struct ChoiceStyle {
        let title: String
        let symbolName: String
        let foreground: UIColor
    }

    struct Appearance {
        var title: String
        var symbolName: String
        var foreground: UIColor
        var collaborationTitle: String?
    }

    var choiceStyles: [String: ChoiceStyle] = [:]
    var onOpenCollaborationMode: (() -> Void)?
    var wantsCollaboration = false

    private var appearance = Appearance(
        title: "",
        symbolName: "shield",
        foreground: .secondaryLabel,
        collaborationTitle: nil
    )
    private var menuSignature = ""
    private var pendingMenu: UIMenu?
    private var pendingSignature: String?
    private var adoptedTitle: String?
    private var appliedKey = ""
    private var isFixingConfiguration = false
    private var symbolCache: [String: UIImage] = [:]

    func sync(appearance: Appearance, menu: UIMenu, signature: String) {
        if isHeld {
            // The open menu owns the source label. Replacing `menu` here
            // restarts it, and a SwiftUI snapshot can still be the previous
            // preset for this frame. Keep the in-flight choice and install
            // the rebuilt menu after the morph finishes.
            if signature != menuSignature {
                pendingMenu = menu
                pendingSignature = signature
            }
            return
        }
        adoptedTitle = nil
        let appearanceChanged = !matches(appearance, self.appearance)
        self.appearance = appearance
        pendingMenu = nil
        pendingSignature = nil
        if appearanceChanged {
            writeConfiguration(suppressAnimation: true)
        }
        guard signature != menuSignature else { return }
        self.menu = menu
        menuSignature = signature
    }

    /// Remember the preset the user chose. The pop-up button itself slides
    /// that action's title into the hidden source; writing configuration here
    /// cancels that morph.
    func adopt(title: String, symbolName: String, foreground: UIColor) {
        appearance.title = title
        appearance.symbolName = symbolName
        appearance.foreground = foreground
        adoptedTitle = title
    }

    /// Collaboration is not a preset. The dismiss completion puts the source
    /// label back if the pop-up button copied this row.
    func keepCurrentPreset() {
        guard configuration?.title == appearance.collaborationTitle else { return }
        writeConfiguration(suppressAnimation: true)
    }

    func fixConfiguration() {
        guard !isFixingConfiguration, !isHeld else { return }
        reconcileFromPopupSelection()
        writeConfiguration(suppressAnimation: true)
    }

    private func writeConfiguration(suppressAnimation: Bool) {
        guard !isFixingConfiguration else { return }
        let resolved = (
            title: appearance.title,
            symbolName: appearance.symbolName,
            foreground: appearance.foreground
        )
        let key = appliedKey(for: resolved)
        var config = configuration ?? .plain()
        let needsWrite = config.title != resolved.title
            || config.image == nil
            || config.indicator != .none
            || appliedKey != key
        guard needsWrite else { return }

        config = styled(title: resolved.title, symbolName: resolved.symbolName, foreground: resolved.foreground)
        isFixingConfiguration = true
        appliedKey = key
        if suppressAnimation {
            UIView.performWithoutAnimation {
                configuration = config
            }
        } else {
            configuration = config
        }
        isFixingConfiguration = false
    }

    override func contextMenuInteraction(
        _ interaction: UIContextMenuInteraction,
        willEndFor configuration: UIContextMenuConfiguration,
        animator: (any UIContextMenuInteractionAnimating)?
    ) {
        super.contextMenuInteraction(interaction, willEndFor: configuration, animator: animator)
        let finish: () -> Void = { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.finishDismiss()
            }
        }
        if let animator {
            animator.addCompletion(finish)
        } else {
            finish()
        }
    }

    private func finishDismiss() {
        adoptedTitle = nil
        // The morph already revealed the new label. Fill a missing shield
        // without clearing the configuration, which would blank it.
        writeConfiguration(suppressAnimation: true)
        if let pendingMenu, let pendingSignature {
            menu = pendingMenu
            menuSignature = pendingSignature
            self.pendingMenu = nil
            self.pendingSignature = nil
        }
        guard wantsCollaboration else { return }
        wantsCollaboration = false
        onOpenCollaborationMode?()
    }

    /// The pop-up button copies the tapped action's title on its own. Follow
    /// that when it is one of the presets, and ignore the collaboration row.
    private func reconcileFromPopupSelection() {
        guard adoptedTitle == nil else { return }
        guard let title = configuration?.title,
              title != appearance.collaborationTitle,
              let choice = choiceStyles[title],
              choice.title != appearance.title else { return }
        appearance.title = choice.title
        appearance.symbolName = choice.symbolName
        appearance.foreground = choice.foreground
    }

    private func styled(
        title: String,
        symbolName: String,
        foreground: UIColor
    ) -> UIButton.Configuration {
        // Borderless + small keeps the composer label at 15pt while the
        // pop-up menu uses the compact control metrics. Plain large is what
        // made the menu much bigger than upstream.
        var config = UIButton.Configuration.borderless()
        config.buttonSize = .mini
        config.title = title
        config.image = symbolImage(symbolName, color: foreground)
        config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(pointSize: 13, weight: .medium)
        config.baseForegroundColor = foreground
        config.indicator = .none
        config.imagePlacement = .leading
        config.imagePadding = 4
        config.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 2, bottom: 0, trailing: 2)
        config.background.backgroundColor = .clear
        config.cornerStyle = .fixed
        config.background.cornerRadius = 0
        config.titleLineBreakMode = .byClipping
        config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak self] incoming in
            var outgoing = incoming
            outgoing.font = LitterFont.uiFont(size: 13, weight: .medium)
            if let color = self?.appearance.foreground {
                outgoing.foregroundColor = color
            }
            return outgoing
        }
        return config
    }

    private func appliedKey(for style: (title: String, symbolName: String, foreground: UIColor)) -> String {
        let resolved = style.foreground.resolvedColor(with: traitCollection)
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        resolved.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        return "\(style.title)|\(style.symbolName)|\(red),\(green),\(blue),\(alpha)"
    }

    private func matches(_ lhs: Appearance, _ rhs: Appearance) -> Bool {
        lhs.title == rhs.title
            && lhs.symbolName == rhs.symbolName
            && lhs.collaborationTitle == rhs.collaborationTitle
            && lhs.foreground.isEqual(rhs.foreground)
    }

    private func symbolImage(_ name: String, color: UIColor) -> UIImage? {
        let key = appliedKey(for: (title: "", symbolName: name, foreground: color))
        if let cached = symbolCache[key] {
            return cached
        }
        let configuration = UIImage.SymbolConfiguration(pointSize: 13, weight: .medium)
        let image = UIImage(systemName: name, withConfiguration: configuration)?
            .withTintColor(color, renderingMode: .alwaysOriginal)
        if let image {
            symbolCache[key] = image
        }
        return image
    }
}
