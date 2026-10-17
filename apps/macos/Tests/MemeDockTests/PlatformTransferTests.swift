import AppKit
import XCTest
@testable import MemeDock

/// Platform-side rules behind drag and Quick Look. The core has its own Rust
/// tests; these cover what the macOS client decides before it reaches the core.
final class PlatformTransferTests: XCTestCase {
    /// A file-backed provider offers exactly the types the drag machinery
    /// expects for a file (its own type and the two URL types) and answers
    /// both a file and data request from the bytes that are already on disk.
    func testFileProviderOffersItsFileAndBytes() throws {
        let source = try makeTemporaryFile(named: "export.png", contents: makePNG())
        let expectedCount = try Data(contentsOf: source).count
        let provider = try XCTUnwrap(NSItemProvider(contentsOf: source))
        XCTAssertTrue(provider.registeredTypeIdentifiers.contains("public.png"))
        XCTAssertTrue(provider.registeredTypeIdentifiers.contains("public.file-url"))
        XCTAssertTrue(provider.registeredTypeIdentifiers.contains("public.url"))

        let fileLoaded = expectation(description: "in-place file")
        provider.loadInPlaceFileRepresentation(forTypeIdentifier: "public.png") { url, _, error in
            XCTAssertNil(error)
            if let url {
                XCTAssertGreaterThan((try? Data(contentsOf: url))?.count ?? 0, 0)
            } else {
                XCTFail("no file URL")
            }
            fileLoaded.fulfill()
        }
        wait(for: [fileLoaded], timeout: 2)

        let dataLoaded = expectation(description: "data")
        provider.loadDataRepresentation(forTypeIdentifier: "public.png") { data, error in
            XCTAssertNil(error)
            XCTAssertEqual(data?.count, expectedCount)
            dataLoaded.fulfill()
        }
        wait(for: [dataLoaded], timeout: 2)

        // The system copies providers while building a drag pasteboard.
        let copy = try XCTUnwrap(provider.copy() as? NSItemProvider)
        let copiedLoaded = expectation(description: "copied data")
        copy.loadDataRepresentation(forTypeIdentifier: "public.png") { data, error in
            XCTAssertNil(error)
            XCTAssertEqual(data?.count, expectedCount)
            copiedLoaded.fulfill()
        }
        wait(for: [copiedLoaded], timeout: 2)
    }

    /// The drag start has to wait for the export: destinations read the file at
    /// once and never wait for a promise.
    func testDragExportWaitCarriesTheResult() throws {
        let expected = try makeTemporaryFile(named: "ready.png", contents: makePNG())
        let ready = DragExport(task: Task {
            try await Task.sleep(nanoseconds: 50_000_000)
            return expected
        })
        guard case let .success(url)? = ready.wait(timeout: 2) else {
            return XCTFail("the export was not ready in time")
        }
        XCTAssertEqual(url, expected)

        let slow = DragExport(task: Task {
            try await Task.sleep(nanoseconds: 2_000_000_000)
            return expected
        })
        XCTAssertNil(slow.wait(timeout: 0.1))
    }

    /// Quick Look reads the type from the file name, so the staged copy must
    /// carry an extension even though the stored original has none.
    func testQuickLookCopyCarriesTheExtension() throws {
        let blob = try makeTemporaryFile(named: "content-hash", contents: makePNG())
        let source = LibraryModel.QuickLookSource(
            path: blob.path,
            mime: "image/png",
            title: "Tired Cat"
        )
        let staged = try XCTUnwrap(LibraryResponder.stage(source))
        defer { try? FileManager.default.removeItem(at: staged.deletingLastPathComponent()) }
        XCTAssertEqual(staged.pathExtension, "png")
        XCTAssertEqual(staged.deletingPathExtension().lastPathComponent, "Tired Cat")
        XCTAssertEqual(try Data(contentsOf: staged), try Data(contentsOf: blob))
    }

    func testQuickLookCopyRejectsAnUnknownType() throws {
        let blob = try makeTemporaryFile(named: "content-hash", contents: Data("x".utf8))
        let source = LibraryModel.QuickLookSource(
            path: blob.path,
            mime: "application/octet-stream",
            title: "Odd"
        )
        XCTAssertNil(LibraryResponder.stage(source))
    }

    // MARK: - Helpers

    private func makePNG() -> Data {
        let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: 8,
            pixelsHigh: 8,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        )!
        return bitmap.representation(using: .png, properties: [:])!
    }

    private func makeTemporaryFile(named name: String, contents: Data) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("MemeDockTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(name)
        try contents.write(to: url)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        return url
    }
}
