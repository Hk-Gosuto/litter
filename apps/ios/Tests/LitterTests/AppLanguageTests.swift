import XCTest
@testable import Litter

final class AppLanguageTests: XCTestCase {
    func testResolvesSupportedLanguages() {
        XCTAssertEqual(AppLanguage.resolve("en", environment: [:]), .english)
        XCTAssertEqual(AppLanguage.resolve("zh-Hans", environment: [:]), .simplifiedChinese)
    }

    func testEnvironmentOverrideWinsForPresentationLocale() {
        XCTAssertEqual(
            AppLanguage.resolve("zh-Hans", environment: ["LITTER_APP_LANGUAGE": "en"]),
            .english
        )
        XCTAssertEqual(
            AppLanguage.resolve("zh-Hans", environment: ["LITTER_APP_LANGUAGE": "unsupported"]),
            .simplifiedChinese
        )
    }

    func testLanguageMetadata() {
        XCTAssertEqual(AppLanguage.english.nativeDisplayName, "English")
        XCTAssertEqual(AppLanguage.simplifiedChinese.nativeDisplayName, "简体中文")
        XCTAssertEqual(AppLanguage.simplifiedChinese.locale.identifier, "zh-Hans")
    }
}
