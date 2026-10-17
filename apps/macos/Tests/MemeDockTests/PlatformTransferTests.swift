import AppKit
import XCTest
import MemeDockBridge
@testable import MemeDock

/// Platform-side rules behind drag and Quick Look. The core has its own Rust
/// tests; these cover what the macOS client decides before it reaches the core.
final class PlatformTransferTests: XCTestCase {
    func testBestCompatibilityIsAdaptiveAndMenusOnlyOfferApplicableChoices() {
        XCTAssertEqual(StickerExport.presets(animated: true), [.bestCompatibility, .original])
        XCTAssertEqual(StickerExport.presets(animated: false), [.bestCompatibility, .original, .whiteBackground, .smallJpeg])
        XCTAssertEqual(StickerExport.preset(storageKey: "compatiblePng", animated: true), .bestCompatibility)
        XCTAssertNil(StickerExport.preset(storageKey: "smallJpeg", animated: true))
        let resolution = StickerExport.resolve(preset: .bestCompatibility, animated: true, firstFrame: false)
        XCTAssertEqual(resolution.preset, .bestCompatibility)
        XCTAssertEqual(resolution.animation, .preserve)
        let options = StickerExport.options(resolution.preset, animation: resolution.animation)
        XCTAssertNil(options.maxEdge) // Core chooses PNG 1024 / GIF 512.
        XCTAssertNil(options.backgroundRgba)
        XCTAssertTrue(options.stripMetadata)
        let first = StickerExport.resolve(preset: .bestCompatibility, animated: true, firstFrame: true)
        XCTAssertEqual(first.preset, .compatiblePng)
        XCTAssertEqual(first.animation, .firstFrame)
        XCTAssertEqual(StickerExport.resolve(preset: .bestCompatibility, animated: false, firstFrame: false).preset, .bestCompatibility)
        XCTAssertNil(StickerExport.effectiveOverride(.smallJpeg, animated: true))
        XCTAssertEqual(StickerExport.resolve(preset: .smallJpeg, animated: true, firstFrame: false).preset, .original)
        XCTAssertEqual(StickerExport.fileExtension(forMime: "image/gif"), "gif")
    }

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

    @MainActor
    func testDragPreparationSerializesCancellationAndKeepsOnlyLatestRequest() async throws {
        let drag = DragExport<String>()
        let started = expectation(description: "first started")
        let completed = expectation(description: "latest completed")
        var release: CheckedContinuation<Void, Never>?
        drag.prepare(key: "a") {
            await withCheckedContinuation { release = $0; started.fulfill() }
            return "stale"
        }
        await fulfillment(of: [started], timeout: 2)
        XCTAssertNil(drag.value(for: "a"))
        drag.prepare(key: "b") { XCTFail("superseded request ran"); return "b" }
        drag.prepare(key: "c") { return "ready" }
        drag.completed = { key, result in
            XCTAssertEqual(key, "c")
            XCTAssertEqual(try? result.get(), "ready")
            completed.fulfill()
        }
        XCTAssertNil(drag.value(for: "c"))
        release?.resume()
        await fulfillment(of: [completed], timeout: 2)
        XCTAssertNil(drag.value(for: "a"))
        XCTAssertEqual(drag.value(for: "c"), "ready")
        drag.invalidate()
        XCTAssertNil(drag.value(for: "c"))
    }

    @MainActor
    func testDragPreparationReportsFailureWithoutReturningAnotherFormat() async {
        let drag = DragExport<String>()
        let completed = expectation(description: "failure")
        drag.completed = { _, result in
            if case .success = result { XCTFail("unexpected fallback") }
            completed.fulfill()
        }
        drag.prepare(key: "gif") { throw NSError(domain: "test", code: 1) }
        await fulfillment(of: [completed], timeout: 2)
        XCTAssertNil(drag.value(for: "gif"))
        XCTAssertNotNil(drag.failure(for: "gif"))
    }

    @MainActor
    func testOutputDefaultsMigrateIndependentlyAndPreserveNewSelections() throws {
        for (legacy, still, animated) in [
            ("compatiblePng", ExportPreset.bestCompatibility, ExportPreset.bestCompatibility),
            ("original", .original, .original),
            ("whiteBackground", .whiteBackground, .original),
            ("smallJpeg", .smallJpeg, .original),
        ] {
            let suite = "MemeDockTests-\(UUID().uuidString)"
            let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
            defer { defaults.removePersistentDomain(forName: suite) }
            for key in ["copyPreset", "sharePreset", "dragPreset"] { defaults.set(legacy, forKey: key) }
            let preferences = Preferences(defaults: defaults)
            XCTAssertEqual(preferences.copyPreset, still)
            XCTAssertEqual(preferences.sharePreset, still)
            XCTAssertEqual(preferences.dragPreset, still)
            XCTAssertEqual(preferences.copyAnimatedPreset, animated)
            XCTAssertEqual(preferences.shareAnimatedPreset, animated)
            XCTAssertEqual(preferences.dragAnimatedPreset, animated)
            preferences.copyAnimatedPreset = .original
            let reloaded = Preferences(defaults: defaults)
            XCTAssertEqual(reloaded.copyAnimatedPreset, .original)
            XCTAssertEqual(reloaded.shareAnimatedPreset, animated)
            XCTAssertEqual(defaults.string(forKey: "copyPreset"), legacy)
        }
        let suite = "MemeDockTests-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set("compatiblePng", forKey: "copyPreset")
        defaults.set("original", forKey: "copyPreset.static")
        let interrupted = Preferences(defaults: defaults)
        XCTAssertEqual(interrupted.copyPreset, .original)
        XCTAssertEqual(interrupted.copyAnimatedPreset, .bestCompatibility)
        XCTAssertEqual(interrupted.dragPreset, .bestCompatibility)
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
