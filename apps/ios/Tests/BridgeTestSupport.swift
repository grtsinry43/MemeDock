import Foundation
import MemeDockBridge
import UIKit
import XCTest
@testable import MemeDock

struct BridgeFixture: Sendable {
    let root: URL
    let paths: AppPaths

    init() {
        root = FileManager.default.temporaryDirectory
            .appendingPathComponent("MemeDockTest-\(UUID().uuidString)", isDirectory: true)
        paths = AppPaths(
            data: root.appendingPathComponent("Library", isDirectory: true),
            cache: root.appendingPathComponent("Cache", isDirectory: true),
            export: root.appendingPathComponent("Exports", isDirectory: true)
        )
    }

    func remove() throws {
        if FileManager.default.fileExists(atPath: root.path) {
            try FileManager.default.removeItem(at: root)
        }
    }
}

@MainActor
func withTestLibrary(
    _ body: (LibraryHandle, BridgeFixture) async throws -> Void
) async throws {
    let fixture = BridgeFixture()
    let library = try await openLibrary(configuration: fixture.paths.configuration())
    do {
        try await body(library, fixture)
        try await library.shutdown()
        try fixture.remove()
    } catch {
        try await library.shutdown()
        try fixture.remove()
        throw error
    }
}

func testQuery() -> StickerQuery {
    StickerQuery(
        requestId: newRequestId(), text: "", collectionId: nil,
        tagIds: [], starred: nil, deleted: false, sort: .recent,
        pageSize: 10, cursor: nil
    )
}

@MainActor
func testPNG(variant: UInt8 = 0) throws -> Data {
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    format.opaque = false
    let renderer = UIGraphicsImageRenderer(size: CGSize(width: 40, height: 20), format: format)
    return renderer.pngData { context in
        UIColor(red: 0.25, green: 0.5, blue: 0.75, alpha: 0.5).setFill()
        context.fill(CGRect(x: 0, y: 0, width: 40, height: 20))
        if variant != 0 {
            // An opaque marker survives alpha premultiplication and PNG quantization.
            UIColor(red: CGFloat(variant) / 255, green: 0.2, blue: 0.3, alpha: 1).setFill()
            context.fill(CGRect(x: 0, y: 0, width: 1, height: 1))
        }
    }
}

@MainActor
func importTestPNG(_ library: LibraryHandle, variant: UInt8 = 0, title: String = "Native Test") async throws -> (String, Data) {
    let bytes = try testPNG(variant: variant)
    let result = try await importTestMedia(library, bytes: bytes, name: "native-test.png", title: title)
    return (result.sticker.id, bytes)
}

@MainActor
func importTestMedia(_ library: LibraryHandle, bytes: Data, name: String, title: String = "Preview Test") async throws -> ImportOutcome {
    let inputTask = try library.createImportInput()
    let input = try await inputTask.awaitResult()
    do {
        try bytes.write(to: URL(fileURLWithPath: input.path()))
    } catch {
        let discard = try library.discardImportInput(input: input)
        try await discard.awaitResult()
        throw error
    }
    let task = try library.importStaged(
        input: input,
        options: ImportOptions(originalName: name, title: title, collectionId: nil)
    )
    let result = try await task.awaitResult()
    return result
}

func assertBridgeCode(
    _ error: any Error, _ expected: ErrorCode,
    file: StaticString = #filePath, line: UInt = #line
) {
    guard case let BridgeError.Failure(code, detail) = error else {
        XCTFail("Unexpected error: \(error)", file: file, line: line)
        return
    }
    XCTAssertEqual(code, expected, file: file, line: line)
    XCTAssertFalse(detail.isEmpty, file: file, line: line)
}

// Real binary fixtures generated with the workspace-pinned image 0.25.10 and
// crates/core/tests/common/image_fixtures.rs. WebP starts with a partial canvas.
enum PreviewFixture: String, CaseIterable {
    case gif, webp, apng

    func bytes() throws -> Data {
        let hex: String
        switch self {
        case .gif:
            hex = "4749463839612800140080000000000000000021ff0b4e45545343415045322e30030102000021f90409040000002c000000002800140080000000c80a1402398c8f99c0edff9494b0d6898fdd2d67ce7918b889136999140aa90adbba081cc91addd9069eeb3ca3dbfd82811f8068440e8349e6d2f7b4190b0021f90409060000002c000000002800140080000000c80a140239848f99c1edff9494b0d6898fdd2d67ce7918b889136999140aa90adbba081cc91addd9069eeb3ca3dbfd82805f8068440e8349e6d2f7b4190b003b"
        case .webp:
            hex = "52494646a401000057454250565038580a00000012000000270000130000414e494d06000000000000000000414e4d46cc000000040000030000130000090000320000025650384cb40000002f13400210cd552022021e88040000000080010c0000000000000000000000000000000000000000000000000000000000000000017820010000000000e71f00000000000000000000000000000000000000000000000400000000000000e0810400000000009c7f00010000000000000000000000000000000000000000000000000000000000800712000000000070fe0100000000000000000000000000000002000000000000000000000000000000f98302414e4d46a4000000040000030000130000090000320000025650384c8b0000002f13400210cd552022021e8804000000008001000000000000000000000000000000000000000000000000030000000000000000017820010000000000e71f00000000000000400000000000000000000000000000000000000000000000e0810400000000009c7f0000000000000000000400000000000000000000000000000000000000000040e4870200"
        case .apng:
            hex = "89504e470d0a1a0a0000000d4948445200000028000000140806000000ff467fbb000000086163544c0000000200000000f38d93700000001a6663544c00000000000000280000001400000000000000000001000a0000d55dfadd00000082494441547801edc003a0245996c6f1ff77ee8dc8cca7724b63ae6ddbb66ddbb66ddbb66d698c9e964aaf9e323322eef976b76a7aa6873b6bd5affef1c6e9cfe67f2e82ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8d7f04c2ce0191d6dbfce10000001a6663544c00000001000000280000001400000000000000000001000a00004e2e10090000008666644154000000027801edc003a0245996c6f1ff77ee8dc8cca7724b63ae6ddbb66ddbb66ddbb66d698c9e964aaf9e323322eef976b76a7aa6873b6bd5affef1c6e9cfe67f2e82ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8de07f3682ffd908fe6723f89f8d7f04c2ce0191b83f41c10000000049454e44ae426082"
        }
        let characters = Array(hex.utf8)
        var bytes = Data()
        for index in stride(from: 0, to: characters.count, by: 2) {
            let pair = String(decoding: characters[index..<index + 2], as: UTF8.self)
            bytes.append(try XCTUnwrap(UInt8(pair, radix: 16)))
        }
        return bytes
    }
}
