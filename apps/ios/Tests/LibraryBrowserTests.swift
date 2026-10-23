import Foundation
import MemeDockBridge
import XCTest
import SwiftUI
import UIKit
@testable import MemeDock

@MainActor
final class LibraryBrowserTests: XCTestCase {
    func testBusinessNotificationsPreserveQueryAndReloadMembership() async throws {
        try await withTestLibrary { library, _ in
            _ = try await importTestPNG(library, variant: 17, title: "Match One")
            let browser = LibraryBrowser(library: library, pageSize: 2)
            browser.filter = .all
            browser.searchText = "Match"
            browser.libraryChanged()
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.count, 1)
            let added = try await importTestPNG(library, variant: 18, title: "Match Two").0
            for _ in 0..<5 { browser.libraryChanged() }
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.searchText, "Match")
            XCTAssertEqual(browser.filter, .all)
            XCTAssertEqual(browser.items.count, 2)
            XCTAssertTrue(browser.items.contains { $0.id == added })
            browser.stop()
        }
    }

    func testRealImportedImagesRenderInNativeGrid() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        var window: UIWindow?
        let originalWindow = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).first { $0.isKeyWindow }
        do {
            let imports = try XCTUnwrap(session.imports)
            let browser = try XCTUnwrap(session.browser)
            let thumbnails = try XCTUnwrap(session.thumbnails)
            let symbols = ["🐱", "🐶", "😂", "🥳", "🥰", "😎", "🤔", "😴", "👍", "❤️", "🎉", "🍉"]
            var sources: [ImportSource] = []
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            for (index, symbol) in symbols.enumerated() {
                let bytes = UIGraphicsImageRenderer(size: CGSize(width: 160, height: 160), format: format).pngData { _ in
                    (symbol as NSString).draw(at: CGPoint(x: 12, y: 10), withAttributes: [.font: UIFont.systemFont(ofSize: 112)])
                }
                let url = fixture.root.appendingPathComponent("sticker-\(index).png")
                try bytes.write(to: url)
                sources.append(.file(url))
            }
            imports.start(sources)
            await imports.waitUntilFinished()
            browser.filter = .all
            await browser.refresh()
            XCTAssertEqual(browser.items.count, symbols.count)
            for item in browser.items { _ = try await thumbnails.image(for: item, pixels: 384) }
            imports.dismissReport()
            let scene = try XCTUnwrap(originalWindow?.windowScene)
            let renderedWindow = UIWindow(windowScene: scene)
            window = renderedWindow
            let host = UIHostingController(rootView: RootView(session: session))
            renderedWindow.rootViewController = host
            renderedWindow.makeKeyAndVisible()
            // Let SwiftUI mount its view tasks and UIKit commit the native navigation layout.
            try await Task.sleep(for: .milliseconds(300))
            host.view.layoutIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: renderedWindow.bounds).image { _ in
                XCTAssertTrue(renderedWindow.drawHierarchy(in: renderedWindow.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "Real imported library - native grid"
            attachment.lifetime = .keepAlways
            add(attachment)
            renderedWindow.isHidden = true
            renderedWindow.rootViewController = nil
            originalWindow?.makeKeyAndVisible()
            await session.shutdown()
            try fixture.remove()
        } catch {
            window?.isHidden = true
            window?.rootViewController = nil
            originalWindow?.makeKeyAndVisible()
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    func testCollectionFilterAndDeletedCollectionFallback() async throws {
        try await withTestLibrary { library, _ in
            let alpha = try await importTestPNG(library, variant: 15, title: "In Collection").0
            _ = try await importTestPNG(library, variant: 16, title: "Outside")
            let collection = try await library.createCollection(name: "Cats", before: nil).awaitResult()
            let detail = try await library.stickerDetail(id: alpha).awaitResult()
            let reference = EntityReference(id: collection.id, generation: collection.lifecycle.generation)
            _ = try await library.setStickerOrganization(
                target: EntityReference(id: alpha, generation: detail.sticker.lifecycle.generation),
                changeCollection: true, collection: reference, tags: nil
            ).awaitResult()
            let browser = LibraryBrowser(library: library)
            await browser.loadCollections()
            XCTAssertEqual(browser.collections.map(\.id), [collection.id])
            browser.filter = .collection(collection.id)
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.map(\.id), [alpha])
            _ = try await library.deleteCollection(target: reference).awaitResult()
            await browser.loadCollections()
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.filter, .all)
            XCTAssertEqual(browser.items.count, 2)
            browser.stop()
        }
    }

    func testPagesAllRealAssetsWithoutDuplicates() async throws {
        try await withTestLibrary { library, _ in
            var ids: Set<String> = []
            for value in UInt8(1)...5 {
                ids.insert(try await importTestPNG(library, variant: value, title: "Sticker \(value)").0)
            }
            XCTAssertEqual(ids.count, 5, "Paging fixtures must be distinct immutable assets")
            let browser = LibraryBrowser(library: library, pageSize: 2)
            browser.filter = .all
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.count, 2)
            XCTAssertTrue(browser.hasMore)
            await browser.loadMore()
            XCTAssertEqual(browser.items.count, 4)
            await browser.loadMore()
            XCTAssertFalse(browser.hasMore)
            XCTAssertEqual(Set(browser.items.map(\.id)), ids)
            XCTAssertEqual(browser.items.count, 5)
            XCTAssertTrue(browser.items.allSatisfy { $0.resource?.asset.hash == $0.id })
            browser.stop()
        }
    }

    func testLatestSearchAndFilterWinOverOutstandingQueries() async throws {
        try await withTestLibrary { library, _ in
            let alpha = try await importTestPNG(library, variant: 1, title: "Alpha Cat").0
            let beta = try await importTestPNG(library, variant: 2, title: "Beta Dog").0
            let browser = LibraryBrowser(library: library, pageSize: 1)
            browser.filter = .all
            browser.searchText = "Alpha"
            await Task.yield()
            browser.searchText = "Beta"
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.map(\.id), [beta])
            XCTAssertFalse(browser.hasMore)
            browser.searchText = "Alpha"
            browser.filter = .starred
            await browser.waitUntilSettled()
            XCTAssertTrue(browser.items.isEmpty)
            browser.filter = .all
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.map(\.id), [alpha])
            browser.stop()
        }
    }

    func testFavoritesAndRecentlyUsedUseCoreSemantics() async throws {
        try await withTestLibrary { library, _ in
            let alpha = try await importTestPNG(library, variant: 3, title: "Alpha").0
            let beta = try await importTestPNG(library, variant: 4, title: "Beta").0
            let detail = try await library.stickerDetail(id: alpha).awaitResult()
            _ = try await library.patchSticker(
                target: EntityReference(id: alpha, generation: detail.sticker.lifecycle.generation),
                edit: StickerEdit(title: nil, note: nil, starred: true)
            ).awaitResult()
            _ = try await library.recordUse(id: alpha, action: .shareLaunched).awaitResult()
            let browser = LibraryBrowser(library: library)
            await browser.refresh()
            XCTAssertEqual(browser.items.first?.id, alpha)
            XCTAssertEqual(Set(browser.items.map(\.id)), [alpha, beta])
            browser.filter = .starred
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.items.map(\.id), [alpha])
            browser.stop()
        }
    }

    func testRefreshKeepsLoadedDepthAndSearchWhileRemovingDeletedAsset() async throws {
        try await withTestLibrary { library, _ in
            for value in UInt8(5)...9 { _ = try await importTestPNG(library, variant: value, title: "Match \(value)") }
            let browser = LibraryBrowser(library: library, pageSize: 2)
            browser.searchText = "Match"
            await browser.waitUntilSettled()
            await browser.loadMore()
            let removed = try XCTUnwrap(browser.items.first)
            _ = try await library.deleteSticker(target: EntityReference(
                id: removed.id, generation: removed.sticker.lifecycle.generation
            )).awaitResult()
            await browser.refresh()
            XCTAssertEqual(browser.searchText, "Match")
            XCTAssertEqual(browser.items.count, 4)
            XCTAssertFalse(browser.items.contains { $0.id == removed.id })
            XCTAssertEqual(Set(browser.items.map(\.id)).count, 4)
            browser.stop()
        }
    }

    func testThumbnailNotificationDoesNotResetPagesAndImportedActionFindsOlderAsset() async throws {
        try await withTestLibrary { library, _ in
            let old = try await importTestPNG(library, variant: 10, title: "Old").0
            for value in UInt8(11)...14 { _ = try await importTestPNG(library, variant: value, title: "New \(value)") }
            let browser = LibraryBrowser(library: library, pageSize: 2)
            browser.filter = .all
            await browser.waitUntilSettled()
            await browser.loadMore()
            let visible = try XCTUnwrap(browser.items.first?.id)
            _ = try await library.requestThumbnail(id: visible, priority: .visible).awaitResult()
            await browser.refreshThumbnail(visible)
            XCTAssertEqual(browser.items.count, 4)
            XCTAssertTrue(browser.hasMore)
            browser.searchText = "No Matches"
            await browser.waitUntilSettled()
            XCTAssertTrue(browser.items.isEmpty)
            let locating = Task { await browser.showImported(old) }
            await Task.yield()
            browser.libraryChanged()
            await locating.value
            await browser.waitUntilSettled()
            XCTAssertEqual(browser.filter, .all)
            XCTAssertEqual(browser.searchText, "")
            XCTAssertEqual(browser.scrollTarget, old)
            XCTAssertTrue(browser.items.contains { $0.id == old })
            browser.stop()
        }
    }

    func testStopRejectsLateSearchAndPageResults() async throws {
        try await withTestLibrary { library, _ in
            _ = try await importTestPNG(library)
            let browser = LibraryBrowser(library: library)
            browser.searchText = "Native"
            browser.stop()
            await browser.waitUntilSettled()
            await browser.refresh()
            XCTAssertTrue(browser.items.isEmpty)
            XCTAssertFalse(browser.hasMore)
            XCTAssertFalse(browser.isLoading)
        }
    }
}
