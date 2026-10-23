import XCTest
@testable import MemeDock

@MainActor
final class AppNavigationTests: XCTestCase {
    func testTabSwitchPreservesIndependentPathsAndImportJumpOnlyResetsHome() async throws {
        try await withTestLibrary { library, _ in
            let imported = try await importTestMedia(library, bytes: testPNG(), name: "navigation.png")
            let detail = StickerDetailRoute(LibraryItem(sticker: imported.sticker, resource: nil))
            let navigation = AppNavigation()
            navigation.homePath = [detail]
            navigation.organizePath = [.group(GroupRoute(filter: .tag("test"), generation: 1)), .sticker(detail)]
            navigation.minePath = [.libraryInfo]
            navigation.selected = .organize
            navigation.selected = .mine
            XCTAssertEqual(navigation.homePath, [detail])
            XCTAssertEqual(navigation.organizePath.count, 2)
            let browser = LibraryBrowser(library: library)
            browser.searchText = "not present"
            await navigation.showImported(imported.sticker.id, browser: browser)
            XCTAssertEqual(navigation.selected, .stickers)
            XCTAssertTrue(navigation.homePath.isEmpty)
            XCTAssertEqual(navigation.organizePath.count, 2)
            XCTAssertEqual(navigation.minePath, [.libraryInfo])
            XCTAssertEqual(browser.filter, .all)
            XCTAssertEqual(browser.searchText, "")
            XCTAssertEqual(browser.scrollTarget, imported.sticker.id)
            browser.stop()
        }
    }
}
