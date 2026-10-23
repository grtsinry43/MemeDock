import Foundation
import ImageIO
import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class ThumbnailStoreTests: XCTestCase {
    func testConcurrentReadersShareImagesWithoutSharingCancellation() async throws {
        try await withTestLibrary { library, _ in
            _ = try await importTestPNG(library)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            let item = LibraryItem(sticker: try XCTUnwrap(page.stickers.first), resource: nil)
            let store = ThumbnailStore(library: library)
            let readers = (0..<8).map { _ in Task { try await store.image(for: item, pixels: 120) } }
            await Task.yield()
            readers[0].cancel()
            _ = await readers[0].result
            var images: [CGImage] = []
            for reader in readers.dropFirst() { images.append(try await reader.value) }
            XCTAssertEqual(images.count, 7)
            XCTAssertTrue(images.allSatisfy { $0.width > 0 && $0.width <= 120 })
            await store.shutdown()
        }
    }

    func testRealThumbnailIsDecodedAndCached() async throws {
        try await withTestLibrary { library, _ in
            let (id, _) = try await importTestPNG(library)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            let metadata = try XCTUnwrap(page.stickers.first)
            let item = LibraryItem(sticker: metadata, resource: page.resources.first { $0.asset.hash == id })
            let store = ThumbnailStore(library: library)
            let first = try await store.image(for: item, pixels: 120)
            let cached = try await store.image(for: item, pixels: 120)
            XCTAssertTrue(first === cached)
            XCTAssertGreaterThan(first.width, 0)
            XCTAssertLessThanOrEqual(first.width, 120)
            await store.clearCache()
            let decoded = try await store.image(for: item, pixels: 80)
            XCTAssertLessThanOrEqual(decoded.width, 80)
            await store.shutdown()
        }
    }

    func testCancelledReaderDoesNotPoisonOtherRequestsAndShutdownDrains() async throws {
        try await withTestLibrary { library, _ in
            _ = try await importTestPNG(library)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            let item = LibraryItem(sticker: try XCTUnwrap(page.stickers.first), resource: nil)
            let store = ThumbnailStore(library: library)
            let cancelled = Task { try await store.image(for: item, pixels: 100) }
            cancelled.cancel()
            do {
                _ = try await cancelled.value
                XCTFail("Cancelled request returned an image")
            } catch is CancellationError {}
            let image = try await store.image(for: item, pixels: 100)
            XCTAssertGreaterThan(image.height, 0)
            let pending = Task { try await store.image(for: item, pixels: 110) }
            await store.shutdown()
            _ = await pending.result
            do {
                _ = try await store.image(for: item, pixels: 100)
                XCTFail("Closed store accepted a request")
            } catch is CancellationError {}
        }
    }

    func testBrokenThumbnailFailsExplicitlyAndRetryUsesCore() async throws {
        try await withTestLibrary { library, fixture in
            let (id, _) = try await importTestPNG(library)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            var resource = try XCTUnwrap(page.resources.first { $0.asset.hash == id })
            resource.thumbnailPath = fixture.root.appendingPathComponent("missing-thumbnail.png").path
            let item = LibraryItem(sticker: try XCTUnwrap(page.stickers.first), resource: resource)
            let store = ThumbnailStore(library: library)
            do {
                _ = try await store.image(for: item, pixels: 100)
                XCTFail("Missing thumbnail was silently accepted")
            } catch {
                XCTAssertFalse(error is CancellationError)
            }
            let retried = try await store.image(for: item, pixels: 100, retry: true)
            XCTAssertGreaterThan(retried.width, 0)
            await store.shutdown()
        }
    }
}
