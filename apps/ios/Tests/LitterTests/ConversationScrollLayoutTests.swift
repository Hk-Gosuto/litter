import XCTest
@testable import Litter

@MainActor
final class ConversationScrollLayoutTests: XCTestCase {
    func testShortConversationStaysAtTop() {
        XCTAssertEqual(ConversationScrollLayout(contentHeight: 280, visibleHeight: 700).bottomOffset, 0)
    }

    func testKeyboardResizeKeepsTargetWithinContent() {
        let fullViewport = ConversationScrollLayout(contentHeight: 1_200, visibleHeight: 700)
        let keyboardViewport = ConversationScrollLayout(contentHeight: 1_200, visibleHeight: 380)

        XCTAssertEqual(fullViewport.bottomOffset, 500)
        XCTAssertEqual(keyboardViewport.bottomOffset, 820)
    }

    func testVisibleAreaAccountsForInsets() {
        let layout = ConversationScrollLayout(contentHeight: 1_200, visibleHeight: 620)
        XCTAssertEqual(layout.bottomOffset, 580)
    }

    func testUnmeasuredViewportDoesNotRequestScroll() {
        XCTAssertNil(ConversationScrollLayout(contentHeight: 280, visibleHeight: 0).bottomOffset)
        XCTAssertNil(ConversationScrollLayout(contentHeight: .infinity, visibleHeight: 700).bottomOffset)
    }

    func testOldThreadCannotDeliverDeferredScrollAfterSwitch() {
        let observation = ConversationScrollObservation()
        XCTAssertTrue(observation.accepts("server::first"))
        observation.layout = ConversationScrollLayout(contentHeight: 1_200, visibleHeight: 700)
        observation.visibleTurnIDs = ["first-turn"]
        observation.followScheduled = true

        observation.activate("server::second")

        XCTAssertFalse(observation.accepts("server::first"))
        XCTAssertTrue(observation.accepts("server::second"))
        XCTAssertNil(observation.layout)
        XCTAssertTrue(observation.visibleTurnIDs.isEmpty)
        XCTAssertFalse(observation.followScheduled)
    }
}
