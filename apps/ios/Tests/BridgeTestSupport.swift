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
        options: ImportOptions(originalName: "native-test.png", title: title, collectionId: nil)
    )
    let result = try await task.awaitResult()
    return (result.sticker.id, bytes)
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
