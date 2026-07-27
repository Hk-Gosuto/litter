import XCTest
@testable import Litter

final class AppLanguageTests: XCTestCase {
    func testResolvesSupportedLanguages() {
        XCTAssertEqual(AppLanguage.resolve("en"), .english)
        XCTAssertEqual(AppLanguage.resolve("zh-Hans"), .simplifiedChinese)
    }

    func testLanguageMetadata() {
        XCTAssertEqual(AppLanguage.english.nativeDisplayName, "English")
        XCTAssertEqual(AppLanguage.simplifiedChinese.nativeDisplayName, "简体中文")
        XCTAssertEqual(AppLanguage.simplifiedChinese.locale.identifier, "zh-Hans")
    }
}
