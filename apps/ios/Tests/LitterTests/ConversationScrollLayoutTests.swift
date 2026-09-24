import XCTest
@testable import Litter

@MainActor
final class ConversationScrollLayoutTests: XCTestCase {
    func testSwitchingThreadsDiscardsVisibleTurns() {
        let observation = ConversationScrollObservation()
        XCTAssertTrue(observation.accepts("server::first"))
        observation.visibleTurnIDs = ["first-turn"]

        observation.activate("server::second")

        XCTAssertFalse(observation.accepts("server::first"))
        XCTAssertTrue(observation.accepts("server::second"))
        XCTAssertTrue(observation.visibleTurnIDs.isEmpty)
    }

    func testProjectionUsesStableRowIdentifiersWhenAnotherTurnAppears() {
        let first = makeTurn("first", text: "Hello")
        let second = makeTurn("second", text: "Answer")
        let initial = project([first])
        let appended = project([first, second])

        XCTAssertEqual(initial.map(\.id), ["row:first:message-first"])
        XCTAssertEqual(appended.map(\.id), ["row:first:message-first", "row:second:message-second"])
        XCTAssertEqual(appended.map(\.turnID), ["first", "second"])
        XCTAssertFalse(appended[0].needsReconfigure(comparedTo: initial[0]))
    }

    func testProjectionReconfiguresChangedExistingMessage() {
        let before = project([makeTurn("first", text: "Draft")])
        let after = project([makeTurn("first", text: "Corrected")])

        XCTAssertEqual(before.map(\.id), after.map(\.id))
        XCTAssertTrue(after[0].needsReconfigure(comparedTo: before[0]))
    }

    func testCollapsedTurnExpandsIntoRowsAndCollapseFooter() {
        let turn = makeTurn("first", text: "Hello", collapsed: true)
        let collapsed = project([turn])
        let expanded = project([turn], expanded: ["first"])

        XCTAssertEqual(collapsed.map(\.id), ["collapsed:first"])
        XCTAssertEqual(expanded.map(\.id), ["row:first:message-first", "footer:first"])
    }

    private func project(_ turns: [TranscriptTurn], expanded: Set<String> = []) -> [ConversationCollectionEntry] {
        ConversationCollectionProjection.build(turns: turns, expandedTurnIDs: expanded) { turn in
            turn.items.map { .item($0) }
        }
    }

    private func makeTurn(_ id: String, text: String, collapsed: Bool = false) -> TranscriptTurn {
        let item = ConversationItem(id: "message-\(id)", content: .user(.init(text: text, images: [])))
        return TranscriptTurn(
            id: id,
            items: [item],
            isLive: false,
            isCollapsedByDefault: collapsed,
            renderDigest: item.renderDigest
        )
    }
}
