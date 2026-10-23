import Foundation
import ImageIO
import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class BridgeMediaTests: XCTestCase {
    func testPNGImportExportAndPersistence() async throws {
        try await withTestLibrary { library, fixture in
            let (id, bytes) = try await importTestPNG(library)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            XCTAssertEqual(detail.asset.mime, "image/png")
            XCTAssertEqual(detail.asset.width, 40)
            XCTAssertEqual(detail.asset.height, 20)
            XCTAssertFalse(detail.asset.animated)
            let originalPath = try XCTUnwrap(detail.originalPath)
            XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: originalPath)), bytes)

            let thumbnail = try await library.requestThumbnail(id: id, priority: .visible).awaitResult()
            let thumbnailSource = try XCTUnwrap(
                CGImageSourceCreateWithURL(URL(fileURLWithPath: thumbnail.path) as CFURL, nil)
            )
            let thumbnailImage = try XCTUnwrap(CGImageSourceCreateImageAtIndex(thumbnailSource, 0, nil))
            XCTAssertGreaterThan(thumbnailImage.width, 0)
            XCTAssertGreaterThan(thumbnailImage.height, 0)

            let originalLease = try await library.exportOriginal(id: id).awaitResult()
            let original = try await library.prepareHandoff(lease: originalLease).awaitResult()
            XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: original.path)), bytes)

            let options = ExportOptions(
                preset: .whiteBackground, maxEdge: nil, backgroundRgba: UInt32.max,
                stripMetadata: true, animation: .preserve
            )
            let lease = try await library.export(id: id, options: options).awaitResult()
            let artifact = try await library.prepareHandoff(lease: lease).awaitResult()
            XCTAssertEqual(artifact.mime, "image/png")
            XCTAssertFalse(artifact.animated)
            let source = try XCTUnwrap(
                CGImageSourceCreateWithURL(URL(fileURLWithPath: artifact.path) as CFURL, nil)
            )
            let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
            XCTAssertEqual(image.width, 40)
            XCTAssertEqual(image.height, 20)
            var pixel = [UInt8](repeating: 0, count: 4)
            try pixel.withUnsafeMutableBytes { buffer in
                let context = try XCTUnwrap(CGContext(
                    data: buffer.baseAddress, width: 1, height: 1,
                    bitsPerComponent: 8, bytesPerRow: 4,
                    space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
                ))
                context.draw(image, in: CGRect(x: 0, y: 0, width: 1, height: 1))
            }
            XCTAssertEqual(pixel[3], 255)
            XCTAssertGreaterThan(pixel[0], 100)
            XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: originalPath)), bytes)
            let protection = try await library.protectClipboard(lease: lease).awaitResult()
            let identity = library.identity()
            try await library.shutdown()

            let reopened = try await openLibrary(configuration: fixture.paths.configuration())
            do {
                XCTAssertEqual(reopened.identity(), identity)
                let page = try await reopened.listStickers(query: testQuery()).awaitResult()
                XCTAssertEqual(page.stickers.map(\.id), [id])
                try await reopened.reconcileClipboard(observed: protection).awaitResult()
                let removed = try await reopened.cleanExportArtifacts().awaitResult()
                XCTAssertEqual(removed, 0)
                XCTAssertTrue(FileManager.default.fileExists(atPath: artifact.path))
                try await reopened.reconcileClipboard(observed: nil).awaitResult()
                try await reopened.shutdown()
            } catch {
                try await reopened.shutdown()
                throw error
            }
        }
    }
}
