import Foundation
import SwiftUI
import UIKit

enum LitterPlatform {
    static let isRunningUnitTests =
        ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil

#if targetEnvironment(macCatalyst)
    static let isCatalyst = true
#else
    static let isCatalyst = false
#endif

    /// `true` only on the unsandboxed Mac Catalyst lane (Developer ID
    /// notarized .dmg). Sandboxed Catalyst (Mac App Store) always sets
    /// `APP_SANDBOX_CONTAINER_ID`, so its absence on a Catalyst process
    /// is a reliable indicator that the App Sandbox is off and we can
    /// spawn child processes (codex app-server, etc.).
    static let isDirectDistMac: Bool = {
        guard isCatalyst else { return false }
        return ProcessInfo.processInfo.environment["APP_SANDBOX_CONTAINER_ID"] == nil
    }()

    /// `true` whenever the process renders as a Mac app — Catalyst or
    /// "Designed for iPad" on Apple Silicon. AppKit-bridge bugs hit
    /// both modes (NSVisualEffectView ignoring `fractionComplete=0`,
    /// NavigationSplitView Liquid Glass material being clobbered by
    /// gradient backdrops, menu-equivalent shortcuts not firing in-view),
    /// so UI workarounds gate on this rather than the compile-time
    /// `targetEnvironment(macCatalyst)` flag — the iOS lane in
    /// "Designed for iPad" mode hits the same AppKit bridge.
    static let rendersAsMacApp: Bool = {
        if isCatalyst { return true }
        return ProcessInfo.processInfo.isiOSAppOnMac
    }()

    static let supportsLocalRuntime = false

    static func bootstrapLocalRuntimeIfNeeded() {
        // All execution is hosted by the connected computer.
    }

    static func defaultLocalWorkingDirectory() -> String {
#if targetEnvironment(macCatalyst)
        return NSHomeDirectory()
#else
        return ishDefaultCwd()
#endif
    }

    static func localRuntimeDisplayName() -> String {
#if targetEnvironment(macCatalyst)
        for candidate in [
            ProcessInfo.processInfo.hostName,
            ProcessInfo.processInfo.environment["HOSTNAME"],
            "Local Mac"
        ] {
            if let displayName = normalizedHostDisplayName(candidate) {
                return displayName
            }
        }
        return AppLanguage.localizedString("Local Mac")
#else
        let device = UIDevice.current.name.trimmingCharacters(in: .whitespacesAndNewlines)
        return device.isEmpty ? AppLanguage.localizedString("This Device") : device
#endif
    }

#if targetEnvironment(macCatalyst)
    private static func normalizedHostDisplayName(_ raw: String?) -> String? {
        guard var value = raw?.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.isEmpty else {
            return nil
        }

        if value.hasSuffix(".local") {
            value.removeLast(".local".count)
        } else if let dotIndex = value.firstIndex(of: ".") {
            value = String(value[..<dotIndex])
        }

        value = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }
#endif

    static func isRegularSurface(horizontalSizeClass: UserInterfaceSizeClass?) -> Bool {
        // Some current iPhone simulator/device combinations report a regular
        // horizontal size class while still rendering as a phone. Treating
        // that as a split-view surface leaves the conversation in a narrow
        // detail column beside a mostly empty sidebar. Phone navigation stays
        // single-column in either orientation; split navigation is for iPad
        // and Mac only.
        if rendersAsMacApp { return true }
        return UIDevice.current.userInterfaceIdiom == .pad && horizontalSizeClass == .regular
    }
}
