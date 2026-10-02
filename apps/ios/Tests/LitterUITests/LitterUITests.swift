import XCTest

final class LitterUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    @MainActor
    func testFollowUpKeepsPreviousTurnVisible() throws {
        let app = conversationDisplayHarnessApp()
        app.launchArguments += ["--ui-test-multiturn", "-collapseTurns", "YES"]
        app.launch()
        XCTAssertTrue(app.staticTexts["HISTORY_MESSAGE_2"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["multiturn.followup"].isHittable)
        app.buttons["multiturn.followup"].tap()
        XCTAssertTrue(app.staticTexts["FOLLOWUP_ANSWER_1"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["HISTORY_MESSAGE_2"].exists)
        app.buttons["Finish"].tap()
        XCTAssertTrue(app.buttons["multiturn.followup"].isHittable)
        app.buttons["multiturn.followup"].tap()
        XCTAssertTrue(app.staticTexts["FOLLOWUP_ANSWER_2"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["FOLLOWUP_ANSWER_1"].exists)
        XCTAssertFalse(app.buttons["Show Less"].exists)
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    @MainActor
    func testLongTurnRemainsScrollableAfterFollowUp() throws {
        let app = conversationDisplayHarnessApp()
        app.launchArguments += ["--ui-test-multiturn", "--ui-test-long-turn", "-collapseTurns", "NO"]
        app.launch()
        XCTAssertTrue(app.staticTexts["HISTORY_MESSAGE_499"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["multiturn.followup"].isHittable)
        app.buttons["multiturn.followup"].tap()
        XCTAssertTrue(app.staticTexts["FOLLOWUP_ANSWER_1"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["HISTORY_MESSAGE_499"].exists)
        (app.collectionViews.firstMatch.exists ? app.collectionViews.firstMatch : app.scrollViews.firstMatch).swipeDown()
        let history = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "HISTORY_MESSAGE_"))
        XCTAssertTrue(history.firstMatch.exists)
        XCTAssertFalse(app.buttons["Show Less"].exists)
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    @MainActor
    func testRichTranscriptStaysInsideReadingColumn() throws {
        let app = conversationDisplayHarnessApp(reasoning: "expanded", commands: "expanded", tools: "expanded")
        app.launchArguments += ["--ui-test-multiturn", "--ui-test-rich"]
        app.launch()

        let prose = app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "This example defines")
        ).firstMatch
        XCTAssertTrue(prose.waitForExistence(timeout: 10))
        attachScreenshot(app, named: "rich-transcript-bottom")
        sleep(1)
        attachScreenshot(app, named: "rich-transcript-bottom-settled")
        let window = app.windows.firstMatch.frame
        // Prose keeps a real gutter on both sides; wide code and tables
        // scroll inside their own boxes instead of widening the column.
        XCTAssertGreaterThanOrEqual(prose.frame.minX, window.minX + 12, "Prose lost its left gutter")
        XCTAssertLessThanOrEqual(prose.frame.maxX, window.maxX - 12, "Prose lost its right gutter")

        let user = app.staticTexts.containing(
            NSPredicate(format: "label BEGINSWITH %@", "Write a markdown answer")
        ).firstMatch
        XCTAssertTrue(user.exists)
        XCTAssertLessThanOrEqual(user.frame.maxX, window.maxX - 12, "User bubble touches the edge")

        app.scrollViews.firstMatch.swipeDown()
        attachScreenshot(app, named: "rich-transcript-top")
        XCTAssertGreaterThanOrEqual(prose.frame.minX, window.minX + 12, "Column moved after scrolling")
    }

    @MainActor
    func testSettingsRootScreenshot() throws {
        let app = conversationDisplayHarnessApp()
        app.launchArguments.append("--ui-test-open-settings")
        app.launch()
        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 10))
        attachScreenshot(app, named: "settings-root")
        app.swipeUp()
        attachScreenshot(app, named: "settings-root-scrolled")
        let appearance = app.descendants(matching: .any)["settings.category.appearance"]
        if appearance.waitForExistence(timeout: 3) {
            if !appearance.isHittable { app.swipeDown() }
            appearance.tap()
            sleep(1)
            attachScreenshot(app, named: "settings-appearance")
        }
    }

    @MainActor
    private func attachScreenshot(_ app: XCUIApplication, named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    @MainActor
    func testConversationDisplaySettingsRowsAreReachable() throws {
        let app = conversationDisplayHarnessApp()
        app.launchArguments.append("--ui-test-open-settings")
        app.launch()

        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 5))
        // Settings is a category list; conversation options live one level in.
        let conversation = app.descendants(matching: .any)["settings.category.conversation"]
        XCTAssertTrue(conversation.waitForExistence(timeout: 10))
        conversation.tap()
        XCTAssertTrue(app.navigationBars["Conversation"].waitForExistence(timeout: 5))
        XCTAssertTrue(findStaticText("Thinking", in: app))
        XCTAssertTrue(findStaticText("Commands", in: app))
        XCTAssertTrue(findStaticText("Tools", in: app))
    }

    @MainActor
    func testHarnessSettingsNavigationEditingAndReadOnlyPolicy() throws {
        let app = conversationDisplayHarnessApp()
        app.launchArguments += ["--ui-test-open-settings", "--ui-test-harness-settings"]
        app.launch()

        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 10))
        XCTAssertTrue(findStaticText("Harnesses", in: app))
        app.staticTexts["Harnesses"].tap()
        XCTAssertTrue(app.navigationBars["Harnesses"].waitForExistence(timeout: 5))
        let runtime = app.buttons["harness.runtime.ui-test-settings-server.pi"]
        XCTAssertTrue(runtime.waitForExistence(timeout: 5))
        runtime.tap()

        let toggleRow = app.buttons["harness.setting.quietStartup"]
        XCTAssertTrue(toggleRow.waitForExistence(timeout: 5))
        toggleRow.tap()
        let toggle = app.switches["Enabled"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 5))
        XCTAssertEqual(toggle.value as? String, "0")
        // SwiftUI exposes the whole Form row as the switch accessibility frame.
        // Tap the trailing switch itself, not the noninteractive row center.
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "value == %@", "1"), object: toggle
        )], timeout: 5), .completed, "The native toggle must change before saving")
        XCTAssertTrue(app.buttons["Save"].isEnabled)
        app.buttons["Save"].tap()
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label CONTAINS %@", "true"), object: toggleRow
        )], timeout: 5), .completed)

        let themeRow = app.buttons["harness.setting.theme"]
        themeRow.tap()
        let choices = app.descendants(matching: .any)["harness.setting.choices"]
        XCTAssertTrue(choices.waitForExistence(timeout: 5))
        choices.tap()
        XCTAssertTrue(app.buttons["dark"].waitForExistence(timeout: 5))
        app.buttons["dark"].tap()
        app.buttons["Save"].tap()
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label CONTAINS %@", "\"dark\""), object: themeRow
        )], timeout: 5), .completed, "The unset enum must save a JSON string")

        app.buttons["harness.setting.unsetName"].tap()
        XCTAssertTrue(app.staticTexts["Unset"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["Save"].isEnabled)
        let input = app.descendants(matching: .any)["harness.setting.input"]
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        XCTAssertNotEqual(input.value as? String, "null")
        input.tap()
        input.typeText("chosen")
        app.buttons["Save"].tap()
        let nameRow = app.buttons["harness.setting.unsetName"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label CONTAINS %@", "\"chosen\""), object: nameRow
        )], timeout: 5), .completed)

        app.buttons["harness.setting.unsetFlag"].tap()
        XCTAssertTrue(app.staticTexts["Unset"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["Save"].isEnabled)
        app.descendants(matching: .any)["harness.setting.choices"].tap()
        app.buttons["Disabled"].tap()
        app.buttons["Save"].tap()
        let flagRow = app.buttons["harness.setting.unsetFlag"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label CONTAINS %@", "false"), object: flagRow
        )], timeout: 5), .completed)

        app.buttons["harness.setting.managedPolicy"].tap()
        XCTAssertTrue(app.navigationBars["Edit setting"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Managed by administrator"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["Save"].isEnabled)
        XCTAssertFalse(app.switches["Enabled"].isEnabled)
        app.buttons["Cancel"].tap()
        XCTAssertTrue(app.buttons["harness.setting.managedPolicy"].waitForExistence(timeout: 5))
    }

    @MainActor
    func testConversationDisplayExpandedModeShowsAllDetails() throws {
        let app = conversationDisplayHarnessApp(reasoning: "expanded", commands: "expanded", tools: "expanded")
        app.launch()

        XCTAssertTrue(app.staticTexts["UITEST_USER_MESSAGE"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["UITEST_ASSISTANT_MESSAGE"].exists)
        XCTAssertTrue(app.staticTexts["UITEST_REASONING_DETAIL"].exists)
        XCTAssertTrue(app.staticTexts["UITEST_COMMAND_OUTPUT"].exists)
        XCTAssertTrue(app.staticTexts["UITEST_TOOL_DETAIL"].exists)
    }

    @MainActor
    func testConversationComposerAcceptsSimulatorKeyboardInput() throws {
        let app = conversationDisplayHarnessApp()
        app.launch()

        XCTAssertTrue(app.buttons["conversation.modelPickerButton"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["Attach"].exists)
        XCTAssertTrue(app.buttons["Mode: Default"].exists)
        let composer = app.textViews["conversation.composerTextView"]
        XCTAssertTrue(composer.exists)
        composer.tap()
        composer.typeText("SIMULATOR_INPUT_OK")
        XCTAssertEqual(composer.value as? String, "SIMULATOR_INPUT_OK")
        XCTAssertTrue(app.buttons["Send"].waitForExistence(timeout: 2))
    }

    @MainActor
    func testConversationComposerKeepsDictationVisibleWithLongModelName() throws {
        let app = conversationDisplayHarnessApp()
        app.launchEnvironment["CODEXIOS_UI_TEST_MODEL_LABEL"] = "HomeLab DeepSeek V4 Flash 0731 Experimental"
        app.launch()

        let dictate = app.buttons["conversation.dictateButton"]
        XCTAssertTrue(dictate.waitForExistence(timeout: 10))
        XCTAssertTrue(dictate.isHittable)
    }

    @MainActor
    func testConversationComposerPreservesRapidKeyboardInput() throws {
        let app = conversationDisplayHarnessApp()
        app.launch()

        let composer = app.textViews["conversation.composerTextView"]
        XCTAssertTrue(composer.waitForExistence(timeout: 10))
        composer.tap()
        let prompt = "Rapid typing keeps every character in the correct order."
        composer.typeText(prompt)
        XCTAssertEqual(composer.value as? String, prompt)
    }

    @MainActor
    func testConversationLaunchPerformance() throws {
        let app = conversationDisplayHarnessApp()
        measure(metrics: [XCTApplicationLaunchMetric()]) {
            app.launch()
            XCTAssertTrue(
                app.staticTexts["Conversation Display Test"].waitForExistence(timeout: 5),
                "Conversation surface did not become interactive after launch"
            )
            app.terminate()
        }
    }

    @MainActor
    func testConversationDisplayCollapsedModeKeepsCompletedDetailsCollapsed() throws {
        let app = conversationDisplayHarnessApp(reasoning: "collapsed", commands: "collapsed", tools: "collapsed")
        app.launch()

        XCTAssertTrue(app.staticTexts["UITEST_USER_MESSAGE"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["UITEST_ASSISTANT_MESSAGE"].exists)
        XCTAssertTrue(app.staticTexts["Thinking"].exists)
        XCTAssertTrue(app.staticTexts["Internal reasoning"].exists)
        XCTAssertTrue(app.staticTexts["printf UITEST_COMMAND_HEADER"].exists)
        XCTAssertTrue(app.staticTexts["uiTest.fixtureTool"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_REASONING_DETAIL"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_COMMAND_OUTPUT"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_TOOL_DETAIL"].exists)
        XCTAssertTrue(app.staticTexts["UITEST_LIVE_COMMAND_OUTPUT"].exists)
    }

    @MainActor
    func testConversationDisplayHiddenModeRemovesDetailRows() throws {
        let app = conversationDisplayHarnessApp(reasoning: "hidden", commands: "hidden", tools: "hidden")
        app.launch()

        XCTAssertTrue(app.staticTexts["UITEST_USER_MESSAGE"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["UITEST_ASSISTANT_MESSAGE"].exists)
        XCTAssertFalse(app.staticTexts["Thinking"].exists)
        XCTAssertFalse(app.staticTexts["Internal reasoning"].exists)
        XCTAssertFalse(app.staticTexts["printf UITEST_COMMAND_HEADER"].exists)
        XCTAssertFalse(app.staticTexts["uiTest.fixtureTool"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_REASONING_DETAIL"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_COMMAND_OUTPUT"].exists)
        XCTAssertFalse(app.staticTexts["UITEST_TOOL_DETAIL"].exists)
    }

    @MainActor
    func testCaptureAppStoreScreenshots() throws {
        let app = XCUIApplication()
        app.launchEnvironment["LITTER_APP_LANGUAGE"] = "en"
        app.launchEnvironment["CODEXIOS_UI_TEST_DISABLE_LOCAL_RUNTIME"] = "1"
        app.launchEnvironment["CODEXIOS_UI_TEST_DISABLE_RUNTIME"] = "1"
        setupSnapshot(app)
        app.launch()

        // Wait for splash to dismiss
        sleep(4)

        // 01 - Home (empty state)
        snapshot("01_Home")

        // 02 - Settings
        let settingsButton = app.buttons["header.settingsButton"]
        if settingsButton.waitForExistence(timeout: 5) {
            settingsButton.tap()
            sleep(1)
            snapshot("02_Settings")

            // Dismiss settings
            app.swipeDown()
            sleep(1)
        }

        // 03 - Discovery
        let connectButton = app.buttons["Connect Server"]
        if connectButton.waitForExistence(timeout: 3), connectButton.isHittable {
            connectButton.tap()
            sleep(2)
            snapshot("03_Discovery")

            // Dismiss discovery
            app.swipeDown()
            sleep(1)
        }
    }

    @MainActor
    func testCaptureScreenshots() throws {
        guard ProcessInfo.processInfo.environment["CODEXIOS_CAPTURE_LIVE_SCREENSHOTS"] == "enabled" else {
            throw XCTSkip("Live discovery screenshots require CODEXIOS_CAPTURE_LIVE_SCREENSHOTS=enabled")
        }
        let app = XCUIApplication()
        app.launchEnvironment["LITTER_APP_LANGUAGE"] = "en"
        app.launchEnvironment["CODEXIOS_UI_TEST_DISABLE_LOCAL_RUNTIME"] = "1"
        app.launchEnvironment["CODEXIOS_UI_TEST_FORCE_DISCOVERY"] = "1"
        setupSnapshot(app)
        app.launch()

        XCTAssertTrue(presentDiscovery(in: app), "Unable to open discovery")
        XCTAssertTrue(identifiedElement("discovery.chooser.kittylitter", in: app).exists)
        XCTAssertTrue(identifiedElement("discovery.chooser.slingshot", in: app).exists)
        XCTAssertTrue(identifiedElement("discovery.chooser.manual", in: app).exists)
        sleep(1)
        snapshot("01DiscoveryChooser")

        let connectedComputer = identifiedElement("discovery.chooser.slingshot", in: app)
        XCTAssertTrue(connectedComputer.isHittable)
        connectedComputer.tap()
        XCTAssertTrue(
            app.navigationBars["Connected Computers"].waitForExistence(timeout: 10),
            "Connected computers screen did not appear"
        )
        sleep(2)
        XCTAssertTrue(app.exists, "App terminated while loading connected computers")
        snapshot("02ConnectedComputers")
    }

    private func presentDiscovery(in app: XCUIApplication) -> Bool {
        if isDiscoveryVisible(in: app) {
            return true
        }

        let primaryConnectButton = app.buttons["Connect Server"]
        if primaryConnectButton.waitForExistence(timeout: 2), primaryConnectButton.isHittable {
            primaryConnectButton.tap()
            return waitForDiscoveryVisible(in: app, timeout: 8)
        }

        let legacyConnectButton = app.buttons["Connect to Server"]
        if legacyConnectButton.waitForExistence(timeout: 2), legacyConnectButton.isHittable {
            legacyConnectButton.tap()
            return waitForDiscoveryVisible(in: app, timeout: 8)
        }

        return waitForDiscoveryVisible(in: app, timeout: 5)
    }

    private func waitForDiscoveryServers(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let codexRows = codexDiscoveryRows(in: app)
        let sshRows = sshDiscoveryRows(in: app)
        let preferredHost = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", ".203"))

        return waitUntil(timeout: timeout) {
            preferredHost.firstMatch.exists || codexRows.firstMatch.exists || sshRows.firstMatch.exists
        }
    }

    private func waitForDiscoveryListToPopulate(
        in app: XCUIApplication,
        timeout: TimeInterval,
        minimumRows: Int
    ) -> Bool {
        let codexRows = codexDiscoveryRows(in: app)
        let sshRows = sshDiscoveryRows(in: app)
        let preferredHost = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", ".203"))
        let scanningLabel = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS[c] %@", "Scanning")
        )

        return waitUntil(timeout: timeout) {
            let totalRows = codexRows.count + sshRows.count
            if totalRows >= minimumRows {
                return true
            }
            if preferredHost.firstMatch.exists && totalRows > 0 && !scanningLabel.firstMatch.exists {
                return true
            }
            return false
        }
    }

    private func selectPreferredDiscoveryServer(in app: XCUIApplication, preferredHostFragment: String) -> Bool {
        let discoveryList = identifiedElement("discovery.list", in: app)
        guard discoveryList.waitForExistence(timeout: 8) else { return false }

        for _ in 0..<5 {
            if tapPreferredDiscoveryRow(in: app, hostFragment: preferredHostFragment) ||
                tapPreferredHostText(in: app, hostFragment: preferredHostFragment) {
                return true
            }
            discoveryList.swipeUp()
        }

        for _ in 0..<5 {
            if tapPreferredDiscoveryRow(in: app, hostFragment: preferredHostFragment) ||
                tapPreferredHostText(in: app, hostFragment: preferredHostFragment) {
                return true
            }
            discoveryList.swipeDown()
        }

        let codexRows = codexDiscoveryRows(in: app)
        if codexRows.firstMatch.waitForExistence(timeout: 4), codexRows.firstMatch.isHittable {
            codexRows.firstMatch.tap()
            return true
        }

        return false
    }

    private func tapPreferredDiscoveryRow(in app: XCUIApplication, hostFragment: String) -> Bool {
        let normalized = hostFragment
            .lowercased()
            .replacingOccurrences(of: ".", with: "_")
            .replacingOccurrences(of: ":", with: "_")

        let query = app.buttons.matching(
            NSPredicate(
                format: "identifier BEGINSWITH %@ AND identifier CONTAINS[c] %@",
                "discovery.server.codex.",
                normalized
            )
        )
        let row = query.firstMatch
        guard row.waitForExistence(timeout: 1), row.isHittable else { return false }
        row.tap()
        return true
    }

    private func tapPreferredHostText(in app: XCUIApplication, hostFragment: String) -> Bool {
        let hostTexts = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", hostFragment))
        let first = hostTexts.firstMatch
        guard first.waitForExistence(timeout: 1), first.isHittable else { return false }
        first.tap()
        return true
    }

    private func waitForDiscoveryVisible(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        waitUntil(timeout: timeout) { isDiscoveryVisible(in: app) }
    }

    private func waitForDiscoveryDismissed(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let discoveryList = identifiedElement("discovery.list", in: app)
        return waitUntil(timeout: timeout) { !discoveryList.exists || !discoveryList.isHittable }
    }

    private func isDiscoveryVisible(in app: XCUIApplication) -> Bool {
        let discoveryList = identifiedElement("discovery.list", in: app)
        let chooser = identifiedElement("discovery.chooser.kittylitter", in: app)
        return (discoveryList.exists && discoveryList.isHittable) ||
            (chooser.exists && chooser.isHittable)
    }

    private func waitForHomeContentReady(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let connectedServerRow = app.descendants(matching: .any).matching(identifier: "home.connectedServerRow")
        let connectButton = app.buttons["Connect Server"]
        return waitUntil(timeout: timeout) {
            (connectedServerRow.firstMatch.exists && connectedServerRow.firstMatch.isHittable) ||
                (connectButton.exists && connectButton.isHittable)
        }
    }

    private func openFirstConnectedServer(in app: XCUIApplication) -> Bool {
        let rows = app.descendants(matching: .any).matching(identifier: "home.connectedServerRow")
        let firstRow = rows.firstMatch
        guard firstRow.waitForExistence(timeout: 8), firstRow.isHittable else { return false }
        firstRow.tap()
        return true
    }

    private func waitForSessionsScreen(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let sessionsContainer = identifiedElement("sessions.container", in: app)
        return waitUntil(timeout: timeout) {
            sessionsContainer.exists && sessionsContainer.isHittable
        }
    }

    private func waitForAnySession(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let rows = app.descendants(matching: .any).matching(identifier: "sessions.sessionRow")
        return waitUntil(timeout: timeout) { rows.firstMatch.exists }
    }

    private func selectFirstSession(in app: XCUIApplication) -> Bool {
        let sessionsContainer = identifiedElement("sessions.container", in: app)
        let rowQuery = app.descendants(matching: .any).matching(identifier: "sessions.sessionRow")

        for _ in 0..<8 {
            let count = min(rowQuery.count, 12)
            if count > 0 {
                for index in 0..<count {
                    let row = rowQuery.element(boundBy: index)
                    if row.exists && row.isHittable {
                        row.tap()
                        return true
                    }
                }
            }
            if sessionsContainer.exists {
                sessionsContainer.swipeUp()
            } else {
                break
            }
        }

        let titles = app.staticTexts.matching(identifier: "sessions.sessionTitle")
        let titleCount = min(titles.count, 12)
        for index in 0..<titleCount {
            let title = titles.element(boundBy: index)
            if title.exists && title.isHittable {
                title.tap()
                return true
            }
        }

        return false
    }

    private func waitForConversationLoaded(in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let backButton = app.buttons["header.homeButton"]
        let sessionsContainer = identifiedElement("sessions.container", in: app)
        return waitUntil(timeout: timeout) {
            backButton.exists && backButton.isHittable && (!sessionsContainer.exists || !sessionsContainer.isHittable)
        }
    }

    private func waitUntil(timeout: TimeInterval, poll: TimeInterval = 0.2, condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() {
                return true
            }
            RunLoop.current.run(until: Date().addingTimeInterval(poll))
        }
        return condition()
    }

    private func identifiedElement(_ identifier: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

    private func codexDiscoveryRows(in app: XCUIApplication) -> XCUIElementQuery {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "discovery.server.codex."))
    }

    private func sshDiscoveryRows(in app: XCUIApplication) -> XCUIElementQuery {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "discovery.server.ssh."))
    }

    @MainActor
    private func conversationDisplayHarnessApp(
        reasoning: String = "collapsed",
        commands: String = "collapsed",
        tools: String = "collapsed"
    ) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments.append("--ui-test-conversation-display")
        app.launchEnvironment["LITTER_APP_LANGUAGE"] = "en"
        app.launchEnvironment["CODEXIOS_UI_TEST_DISABLE_LOCAL_RUNTIME"] = "1"
        app.launchEnvironment["CODEXIOS_UI_TEST_REASONING_MODE"] = reasoning
        app.launchEnvironment["CODEXIOS_UI_TEST_COMMAND_MODE"] = commands
        app.launchEnvironment["CODEXIOS_UI_TEST_TOOL_MODE"] = tools
        return app
    }

    private func findStaticText(_ label: String, in app: XCUIApplication) -> Bool {
        let element = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label == %@ OR label BEGINSWITH %@", label, "\(label),"))
            .firstMatch
        if element.exists {
            return true
        }

        let list = app.collectionViews.firstMatch
        let scrollTarget = list.exists ? list : app.scrollViews.firstMatch
        for _ in 0..<6 {
            scrollTarget.swipeUp()
            if element.waitForExistence(timeout: 1) {
                return true
            }
        }

        return false
    }
}
