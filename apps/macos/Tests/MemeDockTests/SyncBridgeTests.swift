import AppKit
import Foundation
import MemeDockBridge
import XCTest
@testable import MemeDock

/// Runs against the real Axum browser_fixture; never accesses the user's library or Keychain.
final class SyncBridgeTests: XCTestCase {
    func testTwoNativeLibrariesSynchronizeThroughGeneratedBridge() async throws {
        guard let origin = ProcessInfo.processInfo.environment["MEMEDOCK_SYNC_TEST_ORIGIN"] else {
            throw XCTSkip("Set MEMEDOCK_SYNC_TEST_ORIGIN to the local server fixture")
        }
        XCTAssertTrue(origin.hasPrefix("http://127.0.0.1:"))
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("memedock-sync-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        let session = URLSession(configuration: .ephemeral)
        defer { session.invalidateAndCancel() }
        _ = try await request(session, origin, "/admin/login", ["username": "admin", "password": "memedock-test-password"])
        let firstToken = try token(await request(session, origin, "/admin/devices", ["name": "Swift A"]))
        let secondToken = try token(await request(session, origin, "/admin/devices", ["name": "Swift B"]))
        let a = try await openLibrary(configuration: configuration(root.appendingPathComponent("a")))
        let b = try await openLibrary(configuration: configuration(root.appendingPathComponent("b")))
        do {
            let input = try await a.createImportInput().awaitResult()
            let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 2, pixelsHigh: 2, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
            let png = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
            try png.write(to: URL(fileURLWithPath: input.path()))
            let imported = try await a.importStaged(input: input, options: ImportOptions(originalName: "native.png", title: "From Swift", collectionId: nil)).awaitResult()
            let subscription = try a.subscribeSync()
            _ = try await a.connectSync(origin: origin, token: firstToken).awaitResult()
            try await drain(a, token: firstToken)
            _ = try await b.connectSync(origin: origin, token: secondToken).awaitResult()
            try await drain(b, token: secondToken)
            let detail = try await b.stickerDetail(id: imported.sticker.id).awaitResult()
            XCTAssertEqual(detail.sticker.title, "From Swift")
            let current = try await a.stickerDetail(id: imported.sticker.id).awaitResult()
            let reference = EntityReference(id: imported.sticker.id, generation: current.sticker.lifecycle.generation)
            let deleted = try await a.deleteSticker(target: reference).awaitResult()
            try await a.purgeSticker(target: reference, deletedRevision: deleted.lifecycle.revision).awaitResult()
            try await drain(a, token: firstToken)
            try await drain(b, token: secondToken)
            let purged = try await b.stickerDetail(id: imported.sticker.id).awaitResult()
            XCTAssertNotNil(purged.sticker.lifecycle.purgedAt)
            let trash = try await b.trashSnapshot().awaitResult()
            XCTAssertFalse(trash.contains { $0.target.id == imported.sticker.id })
            let again = try await b.createImportInput().awaitResult()
            try png.write(to: URL(fileURLWithPath: again.path()))
            let reimported = try await b.importStaged(input: again, options: ImportOptions(originalName: "new.png", title: "New incarnation", collectionId: nil)).awaitResult()
            XCTAssertEqual(reimported.sticker.lifecycle.generation, current.sticker.lifecycle.generation + 1)
            try await drain(b, token: secondToken)
            try await drain(a, token: firstToken)
            let fresh = try await a.stickerDetail(id: imported.sticker.id).awaitResult()
            XCTAssertEqual(fresh.sticker.title, "New incarnation")
            let paused = try await a.setSyncPaused(paused: true).awaitResult()
            XCTAssertTrue(paused.paused)
            let observed = try await subscription.next()
            XCTAssertEqual(observed?.paused, true)
            _ = try subscription.unsubscribe()
            try await a.shutdown()
            try await b.shutdown()
            let reopened = try await openLibrary(configuration: configuration(root.appendingPathComponent("a")))
            let saved = try await reopened.syncStatus().awaitResult()
            XCTAssertTrue(saved.connected)
            XCTAssertTrue(saved.paused)
            try await reopened.shutdown()
        } catch {
            try? await a.shutdown()
            try? await b.shutdown()
            throw error
        }
    }

    private func configuration(_ root: URL) throws -> LibraryConfiguration {
        LibraryConfiguration(dataDir: root.appendingPathComponent("data").path, cacheDir: root.appendingPathComponent("cache").path, exportDir: root.appendingPathComponent("exports").path, limits: try defaultResourceConfiguration())
    }

    private func drain(_ library: LibraryHandle, token: String) async throws {
        for _ in 0..<16 {
            let state = try await library.synchronize(token: token).awaitResult()
            XCTAssertEqual(state.blocked, 0)
            if state.pending == 0 { return }
        }
        XCTFail("Sync queue did not drain")
    }

    private func token(_ data: Data) throws -> String {
        struct Credential: Decodable { let token: String }
        struct Envelope: Decodable { let data: Credential }
        return try JSONDecoder().decode(Envelope.self, from: data).data.token
    }

    private func request(_ session: URLSession, _ origin: String, _ path: String, _ body: [String: String]) async throws -> Data {
        var request = URLRequest(url: try XCTUnwrap(URL(string: origin + "/api/v1" + path)))
        request.httpMethod = "POST"
        request.setValue(origin, forHTTPHeaderField: "Origin")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await session.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        return data
    }
}
