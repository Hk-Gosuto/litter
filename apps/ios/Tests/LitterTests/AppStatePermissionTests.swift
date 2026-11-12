import XCTest
@testable import Litter

@MainActor
final class AppStatePermissionTests: XCTestCase {
    func testLegacyOnFailureApprovalMigratesToOnRequest() {
        XCTAssertEqual(AppState.normalizedApprovalPolicyWireValue(" on-failure "), "on-request")
        XCTAssertEqual(AppAskForApproval(wireValue: "on-failure"), .onRequest)

        let appState = AppState()
        let key = ThreadKey(serverId: "windows", threadId: "thread-legacy")
        appState.setPermissions(
            approvalPolicy: "on-failure",
            sandboxMode: "workspace-write",
            for: key
        )

        XCTAssertEqual(appState.approvalPolicy(for: key), "on-request")
        XCTAssertEqual(appState.launchApprovalPolicy(for: key), .onRequest)
    }

    func testHydratedThreadPermissionsAreDisplayOnly() {
        let appState = AppState()
        let key = ThreadKey(serverId: "windows", threadId: "thread-1")

        appState.hydratePermissions(from: makeThreadSnapshot(
            key: key,
            effectiveApprovalPolicy: .onRequest,
            effectiveSandboxPolicy: .workspaceWrite(
                writableRoots: [AbsolutePath(value: "relative-root")],
                readOnlyAccess: .fullAccess,
                networkAccess: false,
                excludeTmpdirEnvVar: false,
                excludeSlashTmp: false
            )
        ))

        XCTAssertEqual(appState.approvalPolicy(for: key), "on-request")
        XCTAssertEqual(appState.sandboxMode(for: key), "workspace-write")
        XCTAssertNil(appState.launchApprovalPolicy(for: key))
        XCTAssertNil(appState.launchSandboxMode(for: key))
        XCTAssertNil(appState.turnSandboxPolicy(for: key))
    }

    func testUserThreadPermissionsAreOutboundOverrides() {
        let appState = AppState()
        let key = ThreadKey(serverId: "windows", threadId: "thread-1")

        appState.hydratePermissions(from: makeThreadSnapshot(
            key: key,
            effectiveApprovalPolicy: .onRequest,
            effectiveSandboxPolicy: .workspaceWrite(
                writableRoots: [AbsolutePath(value: "relative-root")],
                readOnlyAccess: .fullAccess,
                networkAccess: false,
                excludeTmpdirEnvVar: false,
                excludeSlashTmp: false
            )
        ))
        appState.setPermissions(
            approvalPolicy: "never",
            sandboxMode: "danger-full-access",
            for: key
        )

        XCTAssertEqual(appState.launchApprovalPolicy(for: key), .never)
        XCTAssertEqual(appState.launchSandboxMode(for: key), .dangerFullAccess)
        XCTAssertEqual(appState.turnSandboxPolicy(for: key), .dangerFullAccess)
    }

    func testNilHydrationKeepsLastConcreteThreadPermissionsDisplayOnly() {
        let appState = AppState()
        let key = ThreadKey(serverId: "windows", threadId: "thread-1")

        appState.hydratePermissions(from: makeThreadSnapshot(
            key: key,
            effectiveApprovalPolicy: .never,
            effectiveSandboxPolicy: .dangerFullAccess
        ))
        appState.hydratePermissions(from: makeThreadSnapshot(
            key: key,
            effectiveApprovalPolicy: nil,
            effectiveSandboxPolicy: nil
        ))

        XCTAssertEqual(appState.approvalPolicy(for: key), "never")
        XCTAssertEqual(appState.sandboxMode(for: key), "danger-full-access")
        XCTAssertNil(appState.launchApprovalPolicy(for: key))
        XCTAssertNil(appState.launchSandboxMode(for: key))
        XCTAssertNil(appState.turnSandboxPolicy(for: key))
    }

    func testPermissionMenuChoicesRoundTripForNewAndExistingSessions() {
        let appState = AppState()
        let existing = ThreadKey(serverId: "server", threadId: "historical")

        for choice in SessionPermissionChoice.allCases {
            appState.setPermissions(
                approvalPolicy: choice.approvalPolicy,
                sandboxMode: choice.sandboxMode,
                for: nil
            )
            XCTAssertEqual(appState.approvalPolicy(for: nil), choice.approvalPolicy)
            XCTAssertEqual(appState.sandboxMode(for: nil), choice.sandboxMode)
            XCTAssertEqual(appState.launchApprovalPolicy(for: nil)?.launchOverrideWireValue, choice.approvalPolicy)
            XCTAssertEqual(sandboxWire(appState.launchSandboxMode(for: nil)), choice.sandboxMode)
            XCTAssertEqual(appState.turnSandboxPolicy(for: nil)?.launchOverrideModeWireValue, choice.sandboxMode)

            appState.setPermissions(
                approvalPolicy: choice.approvalPolicy,
                sandboxMode: choice.sandboxMode,
                for: existing
            )
            XCTAssertEqual(appState.launchApprovalPolicy(for: existing)?.launchOverrideWireValue, choice.approvalPolicy)
            XCTAssertEqual(sandboxWire(appState.launchSandboxMode(for: existing)), choice.sandboxMode)
            XCTAssertEqual(appState.turnSandboxPolicy(for: existing)?.launchOverrideModeWireValue, choice.sandboxMode)
        }
    }

    func testPermissionMenuMatchesKnownPresets() {
        XCTAssertEqual(SessionPermissionChoice.matching(.readOnly), .readOnly)
        XCTAssertEqual(SessionPermissionChoice.matching(.supervised), .auto)
        XCTAssertEqual(SessionPermissionChoice.matching(.fullAccess), .fullAccess)
        XCTAssertNil(SessionPermissionChoice.matching(.custom))
        XCTAssertNil(SessionPermissionChoice.matching(.unknown))
        XCTAssertEqual(SessionPermissionChoice.readOnly.approvalPolicy, "on-request")
        XCTAssertEqual(SessionPermissionChoice.readOnly.sandboxMode, "read-only")
        XCTAssertEqual(SessionPermissionChoice.auto.sandboxMode, "workspace-write")
        XCTAssertEqual(SessionPermissionChoice.fullAccess.approvalPolicy, "never")
        XCTAssertEqual(SessionPermissionChoice.fullAccess.sandboxMode, "danger-full-access")
        XCTAssertEqual(SessionPermissionChoice.readOnly.menuTitleKey, "Read only")
        XCTAssertEqual(SessionPermissionChoice.auto.menuTitleKey, "Auto")
        XCTAssertEqual(SessionPermissionChoice.fullAccess.menuTitleKey, "Full access")
        XCTAssertEqual(SessionPermissionChoice.readOnly.menuDetailKey, "Can read files, asks to edit")
        XCTAssertEqual(SessionPermissionChoice.auto.menuDetailKey, "Edits in this workspace")
        XCTAssertEqual(SessionPermissionChoice.fullAccess.menuDetailKey, "No sandbox, no prompts")
    }
}

final class VoiceTranscriptionRoutingTests: XCTestCase {
    func testCodexAuthModesOnlyUseCompatibleTranscriptionEndpoints() {
        XCTAssertEqual(voiceTranscriptionEndpoint(for: .apiKey), .openAI)
        XCTAssertEqual(voiceTranscriptionEndpoint(for: .chatgpt), .chatGPT)
        XCTAssertEqual(voiceTranscriptionEndpoint(for: .chatgptAuthTokens), .chatGPT)
        XCTAssertEqual(voiceTranscriptionEndpoint(for: .personalAccessToken), .chatGPT)

        XCTAssertNil(voiceTranscriptionEndpoint(for: .headers))
        XCTAssertNil(voiceTranscriptionEndpoint(for: .agentIdentity))
        XCTAssertNil(voiceTranscriptionEndpoint(for: .bedrockApiKey))
        XCTAssertNil(voiceTranscriptionEndpoint(for: nil))
    }
}

private func sandboxWire(_ mode: AppSandboxMode?) -> String? {
    switch mode {
    case .readOnly:
        return "read-only"
    case .workspaceWrite:
        return "workspace-write"
    case .dangerFullAccess:
        return "danger-full-access"
    case nil:
        return nil
    }
}

private func makeThreadSnapshot(
    key: ThreadKey,
    effectiveApprovalPolicy: AppAskForApproval?,
    effectiveSandboxPolicy: AppSandboxPolicy?
) -> AppThreadSnapshot {
    AppThreadSnapshot(
        key: key,
        info: ThreadInfo(
            id: key.threadId,
            title: "Thread",
            model: nil,
            status: .idle,
            preview: "Preview",
            cwd: "C:\\Users\\sigkitten\\dev\\repo",
            path: nil,
            modelProvider: nil,
            agentNickname: nil,
            agentRole: nil,
            parentThreadId: nil,
            forkedFromId: nil,
            agentStatus: nil,
            createdAt: nil,
            updatedAt: nil
        ),
        agentRuntimeKind: .codex,
        collaborationMode: .default,
        model: nil,
        reasoningEffort: nil,
        effectiveApprovalPolicy: effectiveApprovalPolicy,
        effectiveSandboxPolicy: effectiveSandboxPolicy,
        hydratedConversationItems: [],
        queuedFollowUps: [],
        activeTurnId: nil,
        activePlanProgress: nil,
        pendingPlanImplementationPrompt: nil,
        contextTokensUsed: nil,
        modelContextWindow: nil,
        rateLimits: nil,
        realtimeSessionId: nil,
        goal: nil,
        stats: nil,
        tokenUsage: nil,
        olderTurnsCursor: nil,
        initialTurnsLoaded: true
    )
}
