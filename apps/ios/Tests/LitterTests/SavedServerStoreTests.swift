import XCTest
@testable import Litter

@MainActor
final class SavedServerStoreTests: XCTestCase {
    func testRemoteOnlyMigrationKeepsExplicitLoopbackAndPairedServers() {
        let paired = alleycatServer(id: "paired", nodeId: "node-a")
        let loopback = SavedServer(
            id: "forwarded", name: "Tunnel", hostname: "127.0.0.1", port: 8390,
            codexPorts: [8390], sshPort: nil, source: .manual, hasCodexServer: true,
            wakeMAC: nil, preferredConnectionMode: nil, preferredCodexPort: nil,
            sshPortForwardingEnabled: nil, websocketURL: nil, rememberedByUser: true
        )
        let legacyId = alleycatServer(id: "local", nodeId: "127.0.0.1")
        let legacySource = SavedServer(
            id: "renamed-local", name: "This Device", hostname: "127.0.0.1", port: 0,
            codexPorts: [], sshPort: nil, source: .local, hasCodexServer: true,
            wakeMAC: nil, preferredConnectionMode: nil, preferredCodexPort: nil,
            sshPortForwardingEnabled: nil, websocketURL: nil, rememberedByUser: true
        )

        XCTAssertEqual(
            SavedServerStore.remoteServers(from: [legacyId, loopback, legacySource, paired]),
            [loopback, paired]
        )
    }

    func testForgettingAlleycatServerReturnsOnlyUnreferencedNodeToken() {
        let primary = alleycatServer(id: "primary", nodeId: " NODE-A ")
        let duplicate = alleycatServer(id: "duplicate", nodeId: "node-a")
        let independent = alleycatServer(id: "independent", nodeId: "node-b")

        XCTAssertEqual(
            SavedServerStore.orphanedAlleycatNodeIds(
                removing: primary.id,
                from: [primary, duplicate, independent]
            ),
            []
        )
        XCTAssertEqual(
            SavedServerStore.orphanedAlleycatNodeIds(
                removing: independent.id,
                from: [primary, duplicate, independent]
            ),
            ["node-b"]
        )
    }

    private func alleycatServer(id: String, nodeId: String) -> SavedServer {
        SavedServer(
            id: id,
            name: id,
            hostname: nodeId,
            port: nil,
            codexPorts: [],
            sshPort: nil,
            source: .manual,
            hasCodexServer: true,
            wakeMAC: nil,
            preferredConnectionMode: nil,
            preferredCodexPort: nil,
            sshPortForwardingEnabled: nil,
            websocketURL: nil,
            rememberedByUser: true,
            alleycatNodeId: nodeId
        )
    }
}
