import XCTest
@testable import Litter

@MainActor
final class ConversationScrollLayoutTests: XCTestCase {
    func testSwitchingThreadsDiscardsVisibleTurns() {
        var projection = ConversationTranscriptProjection()
        update(&projection, turns: [makeTurn("first", text: "First thread")])
        update(&projection, turns: [makeTurn("second", text: "Second thread")])

        XCTAssertEqual(projection.entries.map(\.turn.id), ["second"])
        XCTAssertEqual(projection.turnIDByEntryID, ["row/message-second": "second"])
    }

    func testProjectionUsesStableRowIdentifiersWhenAnotherTurnAppears() {
        let first = makeTurn("first", text: "Hello")
        let second = makeTurn("second", text: "Answer")
        let initial = project([first])
        let appended = project([first, second])

        XCTAssertEqual(initial.map(\.id), ["row/message-first"])
        XCTAssertEqual(appended.map(\.id), ["row/message-first", "row/message-second"])
        XCTAssertEqual(appended.map(\.turn.id), ["first", "second"])
        XCTAssertEqual(row(in: appended[0]), row(in: initial[0]))
    }

    func testProjectionReconfiguresChangedExistingMessage() {
        var projection = ConversationTranscriptProjection()
        update(&projection, turns: [makeTurn("first", text: "Draft")])
        let before = projection.entries
        update(&projection, turns: [makeTurn("first", text: "Corrected")])
        let after = projection.entries

        XCTAssertEqual(before.map(\.id), after.map(\.id))
        XCTAssertNotEqual(row(in: before[0]), row(in: after[0]))
        XCTAssertEqual(row(in: after[0]), .item(makeTurn("first", text: "Corrected").items[0]))
    }

    func testCollapsedTurnExpandsIntoRowsAndCollapseFooter() {
        let turn = makeTurn("first", text: "Hello", collapsed: true)
        let collapsed = project([turn])
        let expanded = project([turn], expanded: ["first"])

        XCTAssertEqual(collapsed.map(\.id), ["first/summary"])
        XCTAssertEqual(expanded.map(\.id), ["row/message-first", "first/footer"])
    }

    private func project(_ turns: [TranscriptTurn], expanded: Set<String> = []) -> [ConversationTranscriptProjection.Entry] {
        var projection = ConversationTranscriptProjection()
        update(&projection, turns: turns, expanded: expanded)
        return projection.entries
    }

    private func update(_ projection: inout ConversationTranscriptProjection, turns: [TranscriptTurn], expanded: Set<String> = []) {
        projection.update(turns: turns, expandedTurnIDs: expanded,
                          reasoning: .collapsed, commands: .collapsed, tools: .collapsed)
    }

    private func row(in entry: ConversationTranscriptProjection.Entry) -> ConversationTimelineRowDescriptor? {
        guard case .row(let row, _, _) = entry.content else { return nil }
        return row
    }

    private func makeTurn(_ id: String, text: String, collapsed: Bool = false) -> TranscriptTurn {
        let item = ConversationItem(id: "message-\(id)", content: .user(.init(text: text, images: [])), timestamp: Date(timeIntervalSince1970: 0))
        return TranscriptTurn(
            id: id,
            items: [item],
            isLive: false,
            isCollapsedByDefault: collapsed,
            renderDigest: item.renderDigest
        )
    }
}
