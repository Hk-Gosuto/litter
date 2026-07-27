import Foundation

enum AppLanguage: String, CaseIterable, Identifiable {
    case english = "en"
    case simplifiedChinese = "zh-Hans"

    static let storageKey = "litter.appLanguage"

    var id: String { rawValue }

    var locale: Locale {
        Locale(identifier: rawValue)
    }

    var nativeDisplayName: String {
        switch self {
        case .english:
            return "English"
        case .simplifiedChinese:
            return "简体中文"
        }
    }

    static var defaultValue: String {
        inferredSystemLanguage.rawValue
    }

    static var current: AppLanguage {
        guard
            let storedValue = UserDefaults.standard.string(forKey: storageKey),
            let storedLanguage = AppLanguage(rawValue: storedValue)
        else {
            return inferredSystemLanguage
        }
        return storedLanguage
    }

    static func resolve(_ rawValue: String) -> AppLanguage {
        AppLanguage(rawValue: rawValue) ?? inferredSystemLanguage
    }

    static func localizedString(_ key: String) -> String {
        let language = current.rawValue
        guard
            let path = Bundle.main.path(forResource: language, ofType: "lproj"),
            let bundle = Bundle(path: path)
        else {
            return Bundle.main.localizedString(forKey: key, value: key, table: nil)
        }
        return bundle.localizedString(forKey: key, value: key, table: nil)
    }

    private static var inferredSystemLanguage: AppLanguage {
        guard let preferredLanguage = Locale.preferredLanguages.first else {
            return .english
        }
        return preferredLanguage.lowercased().hasPrefix("zh") ? .simplifiedChinese : .english
    }
}
