import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class BridgeLifecycleTests: XCTestCase {
    func testOpenQueryShutdownAndReopen() async throws {
        try await withTestLibrary { library, fixture in
            let identity = library.identity()
            XCTAssertEqual(try library.state(), .open)
            let totals = try await library.spaceStatistics().awaitResult()
            XCTAssertEqual(totals.knownAssets, 0)
            let query = testQuery()
            let page = try await library.listStickers(query: query).awaitResult()
            XCTAssertEqual(page.requestId, query.requestId)
            XCTAssertTrue(page.stickers.isEmpty)
            XCTAssertNil(page.next)
            do {
                let duplicate = try await openLibrary(configuration: fixture.paths.configuration())
                try await duplicate.shutdown()
                XCTFail("Duplicate ownership was accepted")
            } catch {
                assertBridgeCode(error, .alreadyOpen)
            }
            let stream = try library.subscribe()
            try await library.shutdown()
            try await library.shutdown()
            XCTAssertEqual(try library.state(), .closed)
            let event = try await stream.next()
            XCTAssertEqual(event, .closed)
            do {
                _ = try library.spaceStatistics()
                XCTFail("Closed library accepted a query")
            } catch {
                assertBridgeCode(error, .closed)
            }
            let reopened = try await openLibrary(configuration: fixture.paths.configuration())
            XCTAssertEqual(reopened.identity(), identity)
            try await reopened.shutdown()
        }
    }

    func testInvalidInputAndOneShotTaskResult() async throws {
        try await withTestLibrary { library, _ in
            do {
                _ = try library.stickerDetail(id: "invalid")
                XCTFail("Invalid ID was accepted")
            } catch {
                assertBridgeCode(error, .invalidInput)
            }
            let task = try library.spaceStatistics()
            _ = try await task.awaitResult()
            do {
                _ = try await task.awaitResult()
                XCTFail("A task result was consumed twice")
            } catch {
                assertBridgeCode(error, .conflict)
            }
        }
    }

    func testUnsubscribeReleasesWaitingReader() async throws {
        try await withTestLibrary { library, _ in
            let stream = try library.subscribe()
            let reader = Task { try await stream.next() }
            await Task.yield()
            try stream.unsubscribe()
            let result = try await reader.value
            XCTAssertEqual(result, .closed)
            let subsequent = try await stream.next()
            XCTAssertEqual(subsequent, .closed)
        }
    }
}
