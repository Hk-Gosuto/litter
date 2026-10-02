import XCTest
@testable import Litter

final class ConversationTimelineRenderPolicyTests: XCTestCase {
    func testFinishedAssistantInvalidatesWhenHistoryContentChanges() {
        let before = assistantItem(text: "Draft")
        let after = assistantItem(text: "Corrected")

        XCTAssertFalse(ConversationTimelineRowDescriptor.item(before) == .item(after))
        XCTAssertEqual(before.id, after.id)
        XCTAssertNotEqual(before.renderDigest, after.renderDigest)
    }

    func testMathStreamingUsesSnapshotContentUpdates() {
        let before = assistantItem(text: "Inline $x$")
        let after = assistantItem(text: "Inline $x$ and more")

        XCTAssertFalse(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: before.id, text: "Inline $x$"))
        XCTAssertFalse(ConversationTimelineRowDescriptor.item(before) == .item(after))
    }

    func testStreamSwitchesToSnapshotUpdatesWhenMathAppears() {
        let itemId = "math-transition"
        XCTAssertTrue(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: itemId, text: "First sentence."))
        XCTAssertFalse(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: itemId, text: "First sentence. $x$"))
    }

    func testLiveUserMessageStillInvalidatesWhenContentChanges() {
        let before = ConversationItem(id: "item", content: .user(.init(text: "Before", images: [])))
        let after = ConversationItem(id: "item", content: .user(.init(text: "After", images: [])))

        XCTAssertFalse(ConversationTimelineRowDescriptor.item(before) == .item(after))
    }

    func testAssistantLabelChangeInvalidatesTheRowDescriptor() {
        let before = assistantItem(text: "Working")
        let after = ConversationItem(
            id: "item",
            content: .assistant(.init(text: "Working", agentNickname: "sam", agentRole: nil, phase: nil))
        )

        XCTAssertFalse(ConversationTimelineRowDescriptor.item(before) == .item(after))
    }

    private func assistantItem(text: String) -> ConversationItem {
        ConversationItem(
            id: "item",
            content: .assistant(.init(text: text, agentNickname: nil, agentRole: nil, phase: nil))
        )
    }
}
