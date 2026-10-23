import Foundation
import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class ImportCoordinatorTests: XCTestCase {
    func testImportsRealFilesAndReusesDuplicateWithoutChangingOriginal() async throws {
        try await withTestLibrary { library, fixture in
            let bytes = try testPNG()
            let source = fixture.root.appendingPathComponent("source.png")
            try bytes.write(to: source)
            let coordinator = try makeCoordinator(library)
            coordinator.start([.file(source), .file(source)])
            await coordinator.waitUntilFinished()
            XCTAssertEqual(coordinator.items.map(\.status), [.created, .reused])
            XCTAssertEqual(coordinator.added, 1)
            XCTAssertEqual(coordinator.reused, 1)
            XCTAssertFalse(coordinator.hasProblems)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            XCTAssertEqual(page.stickers.count, 1)
            let id = try XCTUnwrap(page.stickers.first?.id)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            XCTAssertEqual(try Data(contentsOf: source), bytes)
            XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: XCTUnwrap(detail.originalPath))), bytes)
            try assertStagingEmpty(fixture)
        }
    }

    func testBadInputDoesNotStopFollowingFileAndCleansStaging() async throws {
        try await withTestLibrary { library, fixture in
            let bad = fixture.root.appendingPathComponent("broken.png")
            let good = fixture.root.appendingPathComponent("good.png")
            try Data("not an image".utf8).write(to: bad)
            try testPNG().write(to: good)
            let coordinator = try makeCoordinator(library)
            coordinator.start([.file(bad), .file(good)])
            await coordinator.waitUntilFinished()
            guard case .failed(let message) = coordinator.items[0].status else {
                return XCTFail("Invalid image was accepted")
            }
            XCTAssertFalse(message.isEmpty)
            XCTAssertEqual(coordinator.items[1].status, .created)
            XCTAssertEqual(coordinator.problems.count, 1)
            XCTAssertTrue(coordinator.cleanupErrors.isEmpty)
            try assertStagingEmpty(fixture)
        }
    }

    func testMissingSourceAndSizeLimitCleanUnsubmittedStaging() async throws {
        try await withTestLibrary { library, fixture in
            let source = fixture.root.appendingPathComponent("large.png")
            try testPNG().write(to: source)
            let coordinator = ImportCoordinator(library: library, maximumBytes: 8)
            coordinator.start([.file(fixture.root.appendingPathComponent("missing.png")), .file(source)])
            await coordinator.waitUntilFinished()
            XCTAssertEqual(coordinator.problems.count, 2)
            XCTAssertTrue(coordinator.items.allSatisfy {
                if case .failed = $0.status { return true }
                return false
            })
            XCTAssertTrue(coordinator.cleanupErrors.isEmpty)
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            XCTAssertTrue(page.stickers.isEmpty)
            try assertStagingEmpty(fixture)
        }
    }

    func testCancellationStopsQueuedItemsAndAllowsNewBatch() async throws {
        try await withTestLibrary { library, fixture in
            let source = fixture.root.appendingPathComponent("source.png")
            try testPNG().write(to: source)
            let coordinator = try makeCoordinator(library)
            coordinator.start([.file(source), .file(source)])
            coordinator.cancel()
            await coordinator.waitUntilFinished()
            XCTAssertEqual(coordinator.items.map(\.status), [.cancelled, .cancelled])
            XCTAssertFalse(coordinator.isRunning)
            coordinator.start([.file(source)])
            await coordinator.waitUntilFinished()
            XCTAssertEqual(coordinator.items.map(\.status), [.created])
            try assertStagingEmpty(fixture)
        }
    }

    func testCancelledCopyChecksBeforeReadingInput() async throws {
        try await withTestLibrary { library, fixture in
            let input = try await library.createImportInput().awaitResult()
            let cancellation = ImportCancellation()
            cancellation.cancel()
            do {
                try await ImportFileIO.copy(
                    from: fixture.root, to: URL(fileURLWithPath: input.path()),
                    maximumBytes: 1024, cancellation: cancellation
                )
                XCTFail("Cancelled copy ran")
            } catch is CancellationError {
                // Cancellation is checked before opening the input or writing output.
            }
            try await library.discardImportInput(input: input).awaitResult()
            try assertStagingEmpty(fixture)
        }
    }

    func testStoppingActiveBatchWaitsForStagingCleanup() async throws {
        try await withTestLibrary { library, fixture in
            let source = fixture.root.appendingPathComponent("source.png")
            try testPNG().write(to: source)
            let coordinator = try makeCoordinator(library)
            coordinator.start([.file(source), .file(source)])
            // Let the owned worker enter its first asynchronous staging operation.
            await Task.yield()
            await coordinator.stopAndWait()
            XCTAssertFalse(coordinator.isRunning)
            XCTAssertTrue(coordinator.items.allSatisfy { $0.status.isFinished })
            // A commit may win the race; results must agree with the persisted library.
            XCTAssertTrue(coordinator.items.allSatisfy {
                [.created, .reused, .cancelled].contains($0.status)
            })
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            XCTAssertEqual(page.stickers.count, coordinator.added)
            XCTAssertTrue(coordinator.cleanupErrors.isEmpty)
            try assertStagingEmpty(fixture)
            let before = coordinator.items.map(\.id)
            coordinator.start([.file(source)])
            XCTAssertEqual(coordinator.items.map(\.id), before, "Closed coordinators cannot admit work")
        }
    }

    func testAdmissionBoundsAndBusyBatchPreservesAcceptedInput() async throws {
        try await withTestLibrary { library, fixture in
            let source = fixture.root.appendingPathComponent("source.png")
            try testPNG().write(to: source)
            let coordinator = try makeCoordinator(library)
            coordinator.start(Array(repeating: .file(source), count: 201))
            XCTAssertNotNil(coordinator.selectionError)
            XCTAssertFalse(coordinator.isRunning)
            coordinator.start([.file(source)])
            let accepted = coordinator.items.map(\.id)
            coordinator.start([.file(fixture.root.appendingPathComponent("missing.png"))])
            XCTAssertEqual(coordinator.items.map(\.id), accepted)
            await coordinator.waitUntilFinished()
            XCTAssertNil(coordinator.selectionError)
            XCTAssertEqual(coordinator.items.map(\.status), [.created])
        }
    }

    func testDeletedDuplicateRequiresExplicitRestoration() async throws {
        try await withTestLibrary { library, fixture in
            let (id, bytes) = try await importTestPNG(library)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            let target = BatchTarget(
                target: EntityReference(id: id, generation: detail.sticker.lifecycle.generation), deletedRevision: nil
            )
            _ = try await library.batch(targets: [target], action: .delete).awaitResult()
            let source = fixture.root.appendingPathComponent("source.png")
            try bytes.write(to: source)
            let coordinator = try makeCoordinator(library)
            coordinator.start([.file(source)])
            await coordinator.waitUntilFinished()
            XCTAssertEqual(coordinator.items.map(\.status), [.restoreRequired])
            let page = try await library.listStickers(query: testQuery()).awaitResult()
            XCTAssertTrue(page.stickers.isEmpty)
        }
    }

    private func makeCoordinator(_ library: LibraryHandle) throws -> ImportCoordinator {
        ImportCoordinator(library: library, maximumBytes: try defaultResourceConfiguration().maxFileBytes)
    }

    private func assertStagingEmpty(_ fixture: BridgeFixture, file: StaticString = #filePath, line: UInt = #line) throws {
        let staging = fixture.paths.data.appendingPathComponent("staging", isDirectory: true)
        let files = try FileManager.default.contentsOfDirectory(at: staging, includingPropertiesForKeys: nil)
        XCTAssertTrue(files.isEmpty, "Unreleased staging inputs: \(files)", file: file, line: line)
    }
}
