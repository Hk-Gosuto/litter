import UIKit
import SwiftUI
import XCTest
@testable import Litter

@MainActor
final class HomeSessionsScrollLayoutTests: XCTestCase {
    func testHomeIllustrationKeepsItsProposedFrameWhenAsyncImageDecodingFinishes() async throws {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "home_cat", withExtension: "webp"))
        let host = UIHostingController(rootView: AlphaAnimatedImageView(fileURL: url).frame(width: 220, height: 124))
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.rootViewController = host
        window.isHidden = false
        defer { window.isHidden = true; window.rootViewController = nil }
        for _ in 0..<100 where findImage(in: host.view) == nil {
            try await Task.sleep(nanoseconds: 10_000_000)
            host.view.layoutIfNeeded()
        }
        host.view.setNeedsLayout()
        host.view.layoutIfNeeded()
        let imageView = try XCTUnwrap(findImage(in: host.view))
        XCTAssertTrue(imageView.clipsToBounds)
        XCTAssertEqual(imageView.bounds.width, 220, accuracy: 0.5)
        XCTAssertEqual(imageView.bounds.height, 124, accuracy: 0.5)
        for _ in 0..<500 where imageView.image == nil {
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTAssertNotNil(imageView.image)
        host.view.setNeedsLayout()
        host.view.layoutIfNeeded()
        XCTAssertEqual(imageView.bounds.width, 220, accuracy: 0.5)
        XCTAssertEqual(imageView.bounds.height, 124, accuracy: 0.5)
    }

    private func findImage(in view: UIView) -> UIImageView? {
        if let image = view as? UIImageView { return image }
        return view.subviews.lazy.compactMap { self.findImage(in: $0) }.first
    }

    func testRestoredScanFooterStaysHiddenUntilInitialLayout() throws {
        let view = HomeSessionsScrollUIView(frame: .zero)
        apply(to: view, zoom: 1)
        let footer = try XCTUnwrap(findFooter(in: view))

        XCTAssertTrue(footer.isHidden)
        XCTAssertTrue(footer.frame.isEmpty)
        XCTAssertTrue(footer.clipsToBounds)

        view.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
        view.setNeedsLayout()
        view.layoutIfNeeded()

        XCTAssertFalse(footer.isHidden)
        XCTAssertEqual(footer.frame.width, 390, accuracy: 0.5)
        XCTAssertGreaterThan(footer.frame.minY, 0)
        XCTAssertEqual(footer.subviews.first?.frame, footer.bounds)
    }

    func testFirstFooterPresentationDoesNotAnimateFromTheOrigin() throws {
        let view = HomeSessionsScrollUIView(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        view.layoutIfNeeded()
        apply(to: view, zoom: 1)
        let footer = try XCTUnwrap(findFooter(in: view))

        XCTAssertFalse(footer.isHidden)
        XCTAssertGreaterThan(footer.frame.minY, 0)
        XCTAssertNil(footer.layer.animation(forKey: "position"))
        XCTAssertNil(footer.layer.animation(forKey: "bounds"))

        apply(to: view, zoom: 2)
        XCTAssertTrue(footer.isHidden)
        apply(to: view, zoom: 1)
        XCTAssertFalse(footer.isHidden)
        XCTAssertGreaterThan(footer.frame.minY, 0)
        XCTAssertNil(footer.layer.animation(forKey: "position"))
    }

    func testSidebarNeverRevealsTheDecorativeFooter() throws {
        let view = HomeSessionsScrollUIView(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        apply(to: view, zoom: 1, showFooter: false)
        let footer = try XCTUnwrap(findFooter(in: view))
        XCTAssertTrue(footer.isHidden)
        XCTAssertTrue(footer.frame.isEmpty)
    }

    private func apply(to view: HomeSessionsScrollUIView, zoom: Int, showFooter: Bool = true) {
        let key = ThreadKey(serverId: "server", threadId: "thread")
        let session = HomeDashboardRecentSession(
            key: key,
            serverId: key.serverId,
            serverDisplayName: "Server",
            agentRuntimeKind: "codex",
            isLocal: false,
            sessionTitle: "Saved session",
            preview: "Preview",
            cwd: "/tmp",
            model: "",
            agentLabel: nil,
            updatedAt: Date(timeIntervalSince1970: 1),
            hasTurnActive: false,
            isResumed: false,
            isSubagent: false,
            isFork: false,
            forkedFromId: nil,
            lineage: nil,
            lastResponsePreview: nil,
            lastResponseTurnId: nil,
            lastUserMessage: nil,
            lastToolLabel: nil,
            stats: nil,
            tokenUsage: nil,
            goal: nil,
            recentToolLog: [],
            lastTurnStart: nil,
            lastTurnEnd: nil
        )
        view.apply(
            sessions: [session],
            pinnedThreadKeys: [],
            hydratingKeys: [],
            cancellingKeys: [],
            openingKey: nil,
            zoomLevel: zoom,
            showCatFooter: showFooter,
            topInset: 48,
            bottomInset: 140,
            textScale: 1,
            themeManager: .shared,
            wallpaperManager: .shared,
            callbacks: .init(
                onOpen: { _ in }, onReply: { _ in }, onHide: { _ in },
                onPin: { _ in }, onUnpin: { _ in }, onCancelTurn: { _ in },
                onDelete: { _ in }, onFork: { _ in }, onShowPiP: { _ in }
            )
        )
    }

    private func findFooter(in view: UIView) -> UIView? {
        if view.accessibilityIdentifier == "home-cat-footer" { return view }
        return view.subviews.lazy.compactMap { self.findFooter(in: $0) }.first
    }
}
