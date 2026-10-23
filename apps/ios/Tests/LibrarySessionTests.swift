import Foundation
import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class LibrarySessionTests: XCTestCase {
    func testShutdownDrainsImportAndReopenUsesNewCoordinator() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        do {
            let source = fixture.root.appendingPathComponent("source.png")
            try testPNG().write(to: source)
            let imports = try XCTUnwrap(session.imports)
            imports.start([.file(source), .file(source)])
            await session.shutdown()
            XCTAssertFalse(imports.isRunning)
            XCTAssertNil(session.imports)
            XCTAssertEqual(session.phase, .closed)
            await session.start()
            let reopened = try XCTUnwrap(session.imports)
            XCTAssertFalse(reopened === imports)
            reopened.start([.file(source)])
            await reopened.waitUntilFinished()
            XCTAssertEqual(reopened.items.map(\.status), [.created])
            await session.shutdown()
            try fixture.remove()
        } catch {
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    func testSessionStartRefreshShutdownAndReopen() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        do {
            XCTAssertEqual(session.phase, .ready)
            let identity = try XCTUnwrap(session.identity)
            await session.start()
            XCTAssertEqual(session.identity, identity)
            let library = try XCTUnwrap(session.library)
            let (id, _) = try await importTestPNG(library)
            await session.refresh()
            // A notification refresh may already own the query. Wait for that
            // real query to finish rather than opening a second core instance.
            for _ in 0..<100 where session.browser?.items.isEmpty == true {
                try await Task.sleep(for: .milliseconds(20))
            }
            XCTAssertEqual(session.browser?.items.map(\.id), [id])
            XCTAssertNil(session.queryError)
            await session.shutdown()
            XCTAssertEqual(session.phase, .closed)
            XCTAssertNil(session.library)
            await session.start()
            XCTAssertEqual(session.phase, .ready)
            XCTAssertEqual(session.identity, identity)
            XCTAssertEqual(session.browser?.items.map(\.id), [id])
            await session.shutdown()
            try fixture.remove()
        } catch {
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    func testInvalidConfigurationFailsWithoutKeepingOwnership() async throws {
        let fixture = BridgeFixture()
        let overlapping = AppPaths(data: fixture.paths.data, cache: fixture.paths.data, export: fixture.paths.export)
        let session = LibrarySession(paths: overlapping)
        await session.start()
        guard case .failed(let detail) = session.phase else {
            XCTFail("Invalid paths did not fail")
            await session.shutdown()
            try fixture.remove()
            return
        }
        XCTAssertFalse(detail.isEmpty)
        XCTAssertNil(session.library)
        await session.shutdown()
        try fixture.remove()
    }

    func testRefreshDuringShutdownDoesNotPolluteReopenedSession() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        do {
            XCTAssertEqual(session.phase, .ready)
            let refresh = Task { await session.refresh() }
            await Task.yield()
            await session.shutdown()
            await session.retry()
            await refresh.value
            await session.refresh()
            XCTAssertEqual(session.phase, .ready)
            XCTAssertNotNil(session.statistics)
            XCTAssertNil(session.queryError)
            await session.shutdown()
            try fixture.remove()
        } catch {
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }
}
