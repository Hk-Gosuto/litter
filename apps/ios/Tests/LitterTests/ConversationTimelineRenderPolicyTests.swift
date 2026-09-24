import XCTest
@testable import Litter

final class ConversationTimelineRenderPolicyTests: XCTestCase {
    func testFinishedAssistantInvalidatesWhenHistoryContentChanges() {
        let before = assistantItem(text: "Draft")
        let after = assistantItem(text: "Corrected")

        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, streamingMessage: false))
        XCTAssertTrue(ConversationTimelineItemRenderPolicy.equivalent(before, after, streamingMessage: true))
    }

    func testLiveUserMessageStillInvalidatesWhenContentChanges() {
        let before = ConversationItem(id: "item", content: .user(.init(text: "Before", images: [])))
        let after = ConversationItem(id: "item", content: .user(.init(text: "After", images: [])))

        XCTAssertFalse(ConversationTimelineItemRenderPolicy.equivalent(before, after, streamingMessage: true))
    }

    private func assistantItem(text: String) -> ConversationItem {
        ConversationItem(
            id: "item",
            content: .assistant(.init(text: text, agentNickname: nil, agentRole: nil, phase: nil))
        )
    }
}
