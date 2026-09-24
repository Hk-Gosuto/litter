import XCTest
@testable import Litter

final class ConversationScrollLayoutTests: XCTestCase {
    func testShortConversationStaysAtTop() {
        XCTAssertEqual(ConversationScrollLayout(contentHeight: 280, viewportHeight: 700).bottomOffset, 0)
    }

    func testKeyboardResizeKeepsTargetWithinContent() {
        let fullViewport = ConversationScrollLayout(contentHeight: 1_200, viewportHeight: 700)
        let keyboardViewport = ConversationScrollLayout(contentHeight: 1_200, viewportHeight: 380)

        XCTAssertEqual(fullViewport.bottomOffset, 500)
        XCTAssertEqual(keyboardViewport.bottomOffset, 820)
    }

    func testUnmeasuredViewportDoesNotRequestScroll() {
        XCTAssertNil(ConversationScrollLayout(contentHeight: 280, viewportHeight: 0).bottomOffset)
        XCTAssertNil(ConversationScrollLayout(contentHeight: .infinity, viewportHeight: 700).bottomOffset)
    }
}
