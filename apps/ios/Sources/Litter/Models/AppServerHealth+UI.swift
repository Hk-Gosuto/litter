import SwiftUI

extension AppServerHealth {
    var displayLabel: String {
        switch self {
        case .connected:
            return AppLanguage.localizedString("Connected")
        case .connecting:
            return AppLanguage.localizedString("Connecting…")
        case .unresponsive:
            return AppLanguage.localizedString("Unresponsive")
        case .disconnected:
            return AppLanguage.localizedString("Disconnected")
        case .unknown:
            return AppLanguage.localizedString("Unknown")
        }
    }

    var accentColor: Color {
        switch self {
        case .connected:
            return LitterTheme.meta
        case .connecting, .unresponsive:
            return LitterTheme.warning
        case .disconnected, .unknown:
            return LitterTheme.textSecondary
        }
    }
}

extension AppServerTransportState {
    var displayLabel: String {
        switch self {
        case .connected:
            return AppLanguage.localizedString("Connected")
        case .connecting:
            return AppLanguage.localizedString("Connecting…")
        case .unresponsive:
            return AppLanguage.localizedString("Unresponsive")
        case .disconnected:
            return AppLanguage.localizedString("Disconnected")
        case .unknown:
            return AppLanguage.localizedString("Unknown")
        }
    }

    var accentColor: Color {
        switch self {
        case .connected:
            return LitterTheme.meta
        case .connecting, .unresponsive:
            return LitterTheme.warning
        case .disconnected, .unknown:
            return LitterTheme.textSecondary
        }
    }
}
