import Foundation
import ImageIO
import XCTest
import MemeDockBridge
import UniformTypeIdentifiers
@testable import MemeDock

@MainActor
final class PreviewDecoderTests: XCTestCase {
    func testGIFLoopBoundariesAndAPNGSinglePlay() async throws {
        try await withTestLibrary { library, _ in
            let gif = try PreviewFixture.gif.bytes()
            let marker = try XCTUnwrap(gif.range(of: Data("NETSCAPE2.0".utf8)))
            var forever = gif
            forever[marker.upperBound + 2] = 0
            forever[marker.upperBound + 3] = 0
            var once = gif
            once.removeSubrange(marker.lowerBound - 3..<marker.upperBound + 5)
            var apng = try PreviewFixture.apng.bytes()
            let animation = try XCTUnwrap(apng.range(of: Data("acTL".utf8)))
            apng.replaceSubrange(animation.upperBound + 4..<animation.upperBound + 8, with: [0, 0, 0, 1])
            let crcBytes = apng[animation.lowerBound..<animation.upperBound + 8]
            var crc = UInt32.max
            for byte in crcBytes {
                crc ^= UInt32(byte)
                for _ in 0..<8 { crc = (crc >> 1) ^ (crc & 1 == 1 ? 0xedb8_8320 : 0) }
            }
            var checksum = (~crc).bigEndian
            let checksumBytes = withUnsafeBytes(of: &checksum) { Data($0) }
            apng.replaceSubrange(animation.upperBound + 8..<animation.upperBound + 12, with: checksumBytes)
            for (bytes, plays) in [(forever, nil as Int?), (once, 1), (apng, 1)] {
                let imported = try await importTestMedia(library, bytes: bytes, name: "loop-fixture")
                let detail = try await library.stickerDetail(id: imported.sticker.id).awaitResult()
                let decoder = PreviewDecoder()
                let token = UUID()
                let info = try await decoder.open(
                    path: XCTUnwrap(detail.originalPath), asset: detail.asset,
                    limits: defaultResourceConfiguration(), pixels: 40, token: token
                )
                XCTAssertEqual(info.totalPlays, plays)
                await decoder.close(token: token)
            }
        }
    }

    func testDecoderPreservesAnimationTimingLoopsAndOriginalBytes() async throws {
        try await withTestLibrary { library, _ in
            for fixture in PreviewFixture.allCases {
                let bytes = try fixture.bytes()
                let imported = try await importTestMedia(library, bytes: bytes, name: "fixture.\(fixture.rawValue)")
                let detail = try await library.stickerDetail(id: imported.sticker.id).awaitResult()
                let path = try XCTUnwrap(detail.originalPath)
                let decoder = PreviewDecoder()
                let token = UUID()
                let info = try await decoder.open(
                    path: path, asset: detail.asset, limits: defaultResourceConfiguration(), pixels: 40, token: token
                )
                XCTAssertEqual(info.frameCount, 2)
                let expected = fixture == .gif ? [0.04, 0.06] : fixture == .webp ? [0.05, 0.05] : [0.1, 0.1]
                for (actual, wanted) in zip(info.delays, expected) { XCTAssertEqual(actual, wanted, accuracy: 0.000001) }
                XCTAssertEqual(info.totalPlays, fixture == .gif ? 3 : nil)
                for index in [0, 1, 0] {
                    let image = try await decoder.frame(at: index, token: token)
                    XCTAssertEqual(image.width, 40)
                    XCTAssertEqual(image.height, 20)
                    let pixels = try rgba(image)
                    let alpha = pixels[(10 * image.width + 10) * 4 + 3]
                    XCTAssertEqual(alpha, fixture == .gif ? (index == 0 ? 255 : 0) : fixture == .webp && index == 1 ? 255 : 128, accuracy: 1)
                }
                await decoder.close(token: token)
                XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: path)), bytes)
            }
        }
    }

    func testDecodeLimitsAndStaleCleanupCannotAffectReplacement() async throws {
        try await withTestLibrary { library, _ in
            let (id, _) = try await importTestPNG(library)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            let path = try XCTUnwrap(detail.originalPath)
            let decoder = PreviewDecoder()
            let first = UUID()
            var limits = try defaultResourceConfiguration()
            limits.maxFramePixels = 1
            do {
                _ = try await decoder.open(path: path, asset: detail.asset, limits: limits, pixels: 40, token: first)
                XCTFail("Pixel budget was ignored")
            } catch PreviewError.resourceLimit {}
            limits = try defaultResourceConfiguration()
            limits.maxFileBytes = 1
            do {
                _ = try await decoder.open(path: path, asset: detail.asset, limits: limits, pixels: 40, token: first)
                XCTFail("File budget was ignored")
            } catch PreviewError.resourceLimit {}
            limits = try defaultResourceConfiguration()
            _ = try await decoder.open(path: path, asset: detail.asset, limits: limits, pixels: 10, token: first)
            let second = UUID()
            _ = try await decoder.open(path: path, asset: detail.asset, limits: limits, pixels: 20, token: second)
            await decoder.close(token: first)
            let image = try await decoder.frame(at: 0, token: second)
            XCTAssertEqual(image.width, 20)
            XCTAssertEqual(image.height, 10)
            do {
                _ = try await decoder.frame(at: 0, token: first)
                XCTFail("Superseded decoder accepted a frame request")
            } catch is CancellationError {}
            await decoder.close(token: second)
        }
    }

    func testStaticPreviewHonorsEXIFOrientation() async throws {
        try await withTestLibrary { library, _ in
            let source = try XCTUnwrap(CGImageSourceCreateWithData(try testPNG() as CFData, nil))
            let original = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
            let bytes = NSMutableData()
            let destination = try XCTUnwrap(CGImageDestinationCreateWithData(bytes, UTType.jpeg.identifier as CFString, 1, nil))
            CGImageDestinationAddImage(destination, original, [kCGImagePropertyOrientation: 6] as CFDictionary)
            XCTAssertTrue(CGImageDestinationFinalize(destination))
            let imported = try await importTestMedia(library, bytes: bytes as Data, name: "rotated.jpg")
            let detail = try await library.stickerDetail(id: imported.sticker.id).awaitResult()
            let decoder = PreviewDecoder()
            let token = UUID()
            _ = try await decoder.open(
                path: XCTUnwrap(detail.originalPath), asset: detail.asset,
                limits: defaultResourceConfiguration(), pixels: 40, token: token
            )
            let image = try await decoder.frame(at: 0, token: token)
            XCTAssertEqual(image.width, 20)
            XCTAssertEqual(image.height, 40)
            await decoder.close(token: token)
        }
    }

    func testSystemDecodesRealAnimatedCanvasesAndTransparency() throws {
        for fixture in PreviewFixture.allCases {
            let source = try XCTUnwrap(CGImageSourceCreateWithData(try fixture.bytes() as CFData, [
                kCGImageSourceShouldCache: false,
            ] as CFDictionary))
            XCTAssertEqual(CGImageSourceGetCount(source), 2, fixture.rawValue)
            for index in 0..<2 {
                let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any])
                XCTAssertEqual((properties[kCGImagePropertyPixelWidth] as? NSNumber)?.intValue, 40)
                XCTAssertEqual((properties[kCGImagePropertyPixelHeight] as? NSNumber)?.intValue, 20)
                let image = try XCTUnwrap(CGImageSourceCreateThumbnailAtIndex(source, index, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceThumbnailMaxPixelSize: 40,
                    kCGImageSourceShouldCacheImmediately: true,
                ] as CFDictionary))
                XCTAssertEqual(image.width, 40, fixture.rawValue)
                XCTAssertEqual(image.height, 20, fixture.rawValue)
                let pixels = try rgba(image)
                let inside = (10 * image.width + 10) * 4
                let outside = (15 * image.width + 30) * 4
                if fixture == .webp {
                    XCTAssertEqual(pixels[outside + 3], 0)
                    XCTAssertEqual(pixels[inside + 3], index == 0 ? 128 : 255, accuracy: 1)
                    XCTAssertGreaterThan(pixels[inside + (index == 0 ? 0 : 1)], 80)
                } else if fixture == .gif {
                    XCTAssertEqual(pixels[inside + 3], index == 0 ? 255 : 0)
                    XCTAssertEqual(pixels[outside + 3], index == 0 ? 0 : 255)
                } else {
                    XCTAssertEqual(pixels[inside + 3], 128, accuracy: 1)
                }
            }
        }
    }

    private func rgba(_ image: CGImage) throws -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
        try bytes.withUnsafeMutableBytes { storage in
            let context = try XCTUnwrap(CGContext(
                data: storage.baseAddress, width: image.width, height: image.height,
                bitsPerComponent: 8, bytesPerRow: image.width * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ))
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        return bytes
    }
}
