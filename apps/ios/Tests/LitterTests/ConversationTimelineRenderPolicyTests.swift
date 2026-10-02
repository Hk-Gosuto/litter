import XCTest
@testable import Litter

final class ConversationTimelineRenderPolicyTests: XCTestCase {
    func testFinishedAssistantInvalidatesWhenHistoryContentChanges() {
        let before = assistantItem(text: "Draft")
        let after = assistantItem(text: "Corrected")

        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, rendererDriven: false))
        XCTAssertTrue(ConversationTimelineItemRenderPolicy.equivalent(before, after, rendererDriven: true))
    }

    func testMathStreamingUsesSnapshotContentUpdates() {
        let before = assistantItem(text: "Inline $x$")
        let after = assistantItem(text: "Inline $x$ and more")

        XCTAssertFalse(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: before.id, text: "Inline $x$"))
        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, rendererDriven: false))
    }

    func testStreamSwitchesToSnapshotUpdatesWhenMathAppears() {
        let itemId = "math-transition"
        XCTAssertTrue(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: itemId, text: "First sentence."))
        XCTAssertFalse(StreamingAssistantBubble.usesStreamingMarkdownRenderer(itemId: itemId, text: "First sentence. $x$"))
    }

    func testLiveUserMessageStillInvalidatesWhenContentChanges() {
        let before = ConversationItem(id: "item", content: .user(.init(text: "Before", images: [])))
        let after = ConversationItem(id: "item", content: .user(.init(text: "After", images: [])))

        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, rendererDriven: true))
    }

    func testStreamingAssistantLabelChangeInvalidatesTheRow() {
        let before = assistantItem(text: "Working")
        let after = ConversationItem(
            id: "item",
            content: .assistant(.init(text: "Working", agentNickname: "sam", agentRole: nil, phase: nil))
        )

        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, rendererDriven: true))
    }

    private func assistantItem(text: String) -> ConversationItem {
        ConversationItem(
            id: "item",
            content: .assistant(.init(text: text, agentNickname: nil, agentRole: nil, phase: nil))
        )
    }
}
