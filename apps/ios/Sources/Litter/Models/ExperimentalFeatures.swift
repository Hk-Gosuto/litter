import Foundation
import Observation

enum LitterFeature: String, CaseIterable, Identifiable {
    case realtimeVoice = "realtime_voice"
    case appleWatch = "apple_watch"
    case terminal = "terminal"

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .realtimeVoice: return AppLanguage.localizedString("Realtime")
        case .appleWatch: return AppLanguage.localizedString("Apple Watch")
        case .terminal: return AppLanguage.localizedString("Terminal")
        }
    }

    var description: String {
        switch self {
        case .realtimeVoice:
            return AppLanguage.localizedString("Show the realtime voice launcher on the home screen.")
        case .appleWatch:
            return AppLanguage.localizedString("Push server, task, and approval state to a paired Apple Watch. Requires the Litter watch app to be installed.")
        case .terminal:
            return AppLanguage.localizedString("Show the local and remote terminal launcher on the home screen.")
        }
    }

    var defaultEnabled: Bool {
        switch self {
        case .realtimeVoice: return true
        case .terminal: return false
        case .appleWatch:
            // Default on now that the watch app is embedded again. The bridge
            // still no-ops when WatchConnectivity is unavailable.
            return true
        }
    }
}

@Observable
final class ExperimentalFeatures {
    static let shared = ExperimentalFeatures()

    @ObservationIgnored private let key = "litter.experimentalFeatures"
    private var overrides: [String: Bool]

    private init() {
        overrides = UserDefaults.standard.dictionary(forKey: key) as? [String: Bool] ?? [:]
    }

    private func persistOverrides() {
        UserDefaults.standard.set(overrides, forKey: key)
    }

    func isEnabled(_ feature: LitterFeature) -> Bool {
        overrides[feature.rawValue] ?? feature.defaultEnabled
    }

    func setEnabled(_ feature: LitterFeature, _ value: Bool) {
        var map = overrides
        if value == feature.defaultEnabled {
            map.removeValue(forKey: feature.rawValue)
        } else {
            map[feature.rawValue] = value
        }
        overrides = map
        persistOverrides()
    }

}
