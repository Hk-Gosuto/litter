import XCTest
@testable import Litter

@MainActor
final class PendingUserInputPresentationTests: XCTestCase {
    func testClosingAQuestionShowsTheNextQuestionAndKeepsDrafts() {
        let state = AppState()
        let first = request("first")
        let second = request("second")
        state.pendingUserInputNotes[first.id] = ["answer": "Unsent first answer"]
        state.pendingUserInputSelections[second.id] = ["answer": "B"]

        XCTAssertEqual(state.visiblePendingUserInput(in: [first, second])?.id, first.id)
        state.dismissPendingUserInput(id: first.id)
        XCTAssertEqual(state.visiblePendingUserInput(in: [first, second])?.id, second.id)
        XCTAssertEqual(state.pendingUserInputNotes[first.id]?["answer"], "Unsent first answer")

        state.dismissPendingUserInput(id: second.id)
        let collapsed = state.visiblePendingUserInput(in: [first, second])
        XCTAssertEqual(collapsed?.id, first.id, "Keep a collapsed entry available for reopening")
        XCTAssertTrue(state.isPendingUserInputDismissed(id: first.id))
        state.restorePendingUserInput(id: first.id)
        XCTAssertEqual(state.visiblePendingUserInput(in: [first, second])?.id, first.id)
        XCTAssertFalse(state.isPendingUserInputDismissed(id: first.id))
        XCTAssertEqual(state.pendingUserInputSelections[second.id]?["answer"], "B")
    }

    func testResolvedRequestsCannotLeaveABlockingCollapsedEntry() {
        let state = AppState()
        let old = request("old")
        let current = request("current")
        state.dismissPendingUserInput(id: old.id)
        XCTAssertEqual(state.visiblePendingUserInput(in: [current])?.id, current.id)
        XCTAssertNil(state.visiblePendingUserInput(in: []))
    }

    func testEveryCollapsedQuestionCanBeReopenedWithoutAnsweringAnotherQuestion() {
        let state = AppState()
        let requests = [request("one"), request("two"), request("three")]
        for request in requests { state.dismissPendingUserInput(id: request.id) }
        for expected in requests + requests {
            let visible = state.visiblePendingUserInput(in: requests)
            XCTAssertEqual(visible?.id, expected.id)
            XCTAssertTrue(state.isPendingUserInputDismissed(id: expected.id))
            state.restorePendingUserInput(id: expected.id)
            XCTAssertEqual(state.visiblePendingUserInput(in: requests)?.id, expected.id)
            state.dismissPendingUserInput(id: expected.id)
        }
    }

    private func request(_ id: String) -> PendingUserInputRequest {
        PendingUserInputRequest(id: id, serverId: "s", threadId: "t", turnId: "turn", itemId: id,
                                questions: [], requesterAgentNickname: nil, requesterAgentRole: nil, isAsync: true)
    }
}
