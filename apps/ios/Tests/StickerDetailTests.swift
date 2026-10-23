import Foundation
import MemeDockBridge
import SwiftUI
import UIKit
import XCTest
@testable import MemeDock

@MainActor
final class StickerDetailTests: XCTestCase {
    func testReleasingActiveDetailStopsInfiniteAnimation() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let imported = try await importTestMedia(library, bytes: PreviewFixture.webp.bytes(), name: "infinite.webp")
            let route = StickerDetailRoute(LibraryItem(sticker: imported.sticker, resource: nil))
            var model: StickerDetailModel? = try XCTUnwrap(session.makeDetail(for: route))
            weak var observed: StickerDetailModel?
            observed = model
            let playback = try XCTUnwrap(model?.playback)
            await model?.load(pixels: 200)
            XCTAssertEqual(playback.state, .playing)
            model = nil
            try await eventually { observed == nil }
            try await eventually { playback.frame == nil && playback.state == .idle }
        }
    }

    func testCorruptOriginalFailsAndExplicitRetryDecodesRepairedFile() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let (id, bytes) = try await importTestPNG(library)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            let path = URL(fileURLWithPath: try XCTUnwrap(detail.originalPath))
            // Simulate disk corruption only inside this disposable fixture library.
            try Data(repeating: 0, count: bytes.count).write(to: path)
            let route = StickerDetailRoute(LibraryItem(sticker: detail.sticker, resource: nil))
            let model = try XCTUnwrap(session.makeDetail(for: route))
            await model.load(pixels: 100)
            guard case .failed = model.playback.state else { return XCTFail("Corrupt preview was accepted") }
            XCTAssertNil(model.playback.frame)
            try bytes.write(to: path)
            await model.load(pixels: 100)
            XCTAssertEqual(model.playback.state, .still)
            XCTAssertNotNil(model.playback.frame)
        }
    }

    func testRealDetailLoadsAndMetadataRefreshKeepsPreview() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let (id, _) = try await importTestPNG(library)
            await session.refresh()
            let item = try XCTUnwrap(session.browser?.items.first { $0.id == id })
            let model = try XCTUnwrap(session.makeDetail(for: StickerDetailRoute(item)))
            await model.load(pixels: 400)
            XCTAssertEqual(model.phase, .ready)
            XCTAssertEqual(model.playback.state, .still)
            let image = try XCTUnwrap(model.playback.frame)
            let updated = try await library.patchSticker(
                target: EntityReference(id: id, generation: item.sticker.lifecycle.generation),
                edit: StickerEdit(title: "Updated Title", note: "A note", starred: nil)
            ).awaitResult()
            model.scheduleReload()
            await model.waitUntilLoaded()
            XCTAssertEqual(model.detail?.sticker.title, updated.title)
            XCTAssertTrue(model.playback.frame === image)
            await session.shutdown()
            XCTAssertEqual(model.phase, .closed)
            XCTAssertNil(model.playback.frame)
        }
    }

    func testAnimationPauseResumeReducedMotionAndFiniteLoops() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let imported = try await importTestMedia(library, bytes: PreviewFixture.gif.bytes(), name: "loop.gif")
            let item = LibraryItem(sticker: imported.sticker, resource: nil)
            let model = try XCTUnwrap(session.makeDetail(for: StickerDetailRoute(item)))
            model.setPresentation(active: true, reduceMotion: true)
            await model.load(pixels: 100)
            XCTAssertEqual(model.playback.state, .paused)
            XCTAssertNotNil(model.playback.frame)
            await model.playback.toggle()
            XCTAssertEqual(model.playback.state, .playing)
            await model.playback.toggle()
            XCTAssertEqual(model.playback.state, .paused)
            XCTAssertNotNil(model.playback.frame)
            model.setPresentation(active: false, reduceMotion: true)
            XCTAssertNil(model.playback.frame)
            model.setPresentation(active: true, reduceMotion: true)
            await model.playback.waitUntilPrepared()
            XCTAssertEqual(model.playback.state, .paused)
            XCTAssertNotNil(model.playback.frame)
            await model.playback.toggle()
            try await eventually { model.playback.state == .finished }
            XCTAssertNotNil(model.playback.frame)
            await model.playback.toggle()
            XCTAssertEqual(model.playback.state, .playing)
            model.suspend()
            XCTAssertNil(model.playback.frame)
        }
    }

    func testMissingOriginalIsExplicitAndDeletedGenerationCannotBeReused() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let (id, bytes) = try await importTestPNG(library)
            let detail = try await library.stickerDetail(id: id).awaitResult()
            let item = LibraryItem(sticker: detail.sticker, resource: nil)
            let route = StickerDetailRoute(item)
            let model = try XCTUnwrap(session.makeDetail(for: route))
            try FileManager.default.removeItem(atPath: XCTUnwrap(detail.originalPath))
            await model.load(pixels: 100)
            XCTAssertEqual(model.phase, .ready)
            XCTAssertNotNil(model.originalError)
            XCTAssertNil(model.playback.frame)
            let target = EntityReference(id: id, generation: detail.sticker.lifecycle.generation)
            let deleted = try await library.deleteSticker(target: target).awaitResult()
            await model.load(pixels: 100)
            guard case .unavailable = model.phase else { return XCTFail("Deleted sticker remained available") }
            try await library.purgeSticker(target: target, deletedRevision: deleted.lifecycle.revision).awaitResult()
            let replacement = try await importTestMedia(library, bytes: bytes, name: "replacement.png")
            XCTAssertGreaterThan(replacement.sticker.lifecycle.generation, route.generation)
            await model.load(pixels: 100)
            guard case .unavailable = model.phase else { return XCTFail("Old route adopted a replacement generation") }
            let next = try XCTUnwrap(session.makeDetail(for: StickerDetailRoute(
                LibraryItem(sticker: replacement.sticker, resource: nil)
            )))
            await next.load(pixels: 100)
            XCTAssertEqual(next.phase, .ready)
            XCTAssertNotNil(next.playback.frame)
        }
    }

    func testClosingDuringLoadDiscardsLateResultsAndReleasesModel() async throws {
        try await withSession { session, _ in
            let library = try XCTUnwrap(session.library)
            let imported = try await importTestMedia(library, bytes: PreviewFixture.webp.bytes(), name: "moving.webp")
            let route = StickerDetailRoute(LibraryItem(sticker: imported.sticker, resource: nil))
            var model = try XCTUnwrap(session.makeDetail(for: route)) as StickerDetailModel?
            weak var observed: StickerDetailModel?
            observed = model
            let loading = Task { await model?.load(pixels: 200) }
            await Task.yield()
            await session.shutdown()
            await loading.value
            XCTAssertEqual(model?.phase, .closed)
            XCTAssertNil(model?.playback.frame)
            model = nil
            try await eventually { observed == nil }
        }
    }

    func testDetailRendersRealPreviewAndMetadata() async throws {
        try await withSession { session, fixture in
            let library = try XCTUnwrap(session.library)
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            let bytes = UIGraphicsImageRenderer(size: CGSize(width: 320, height: 320), format: format).pngData { _ in
                ("🥳" as NSString).draw(at: CGPoint(x: 16, y: 12), withAttributes: [.font: UIFont.systemFont(ofSize: 260)])
            }
            let imported = try await importTestMedia(library, bytes: bytes, name: "celebration.png", title: "A little celebration")
            _ = try await library.patchSticker(
                target: EntityReference(id: imported.sticker.id, generation: imported.sticker.lifecycle.generation),
                edit: StickerEdit(title: nil, note: "Ready for a happy moment.", starred: nil)
            ).awaitResult()
            let route = StickerDetailRoute(LibraryItem(sticker: imported.sticker, resource: nil))
            let originalWindow = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
                .flatMap(\.windows).first { $0.isKeyWindow }
            let scene = try XCTUnwrap(originalWindow?.windowScene)
            let window = UIWindow(windowScene: scene)
            defer {
                window.isHidden = true
                window.rootViewController = nil
                originalWindow?.makeKeyAndVisible()
            }
            let host = UIHostingController(rootView: NavigationStack {
                StickerDetailView(session: session, route: route)
                    .environment(\.scenePhase, .active)
            })
            window.rootViewController = host
            window.makeKeyAndVisible()
            // Allow native navigation layout and the real asynchronous preview to present.
            try await Task.sleep(for: .milliseconds(400))
            host.view.layoutIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "Real sticker detail"
            attachment.lifetime = .keepAlways
            add(attachment)
            let rendered = try XCTUnwrap(image.cgImage)
            var pixels = [UInt8](repeating: 0, count: 64 * 64 * 4)
            try pixels.withUnsafeMutableBytes { buffer in
                let context = try XCTUnwrap(CGContext(
                    data: buffer.baseAddress, width: 64, height: 64, bitsPerComponent: 8, bytesPerRow: 64 * 4,
                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
                ))
                context.draw(rendered, in: CGRect(x: 0, y: 0, width: 64, height: 64))
            }
            // Inspect the preview region, excluding the blue File Information control.
            let colored = (0..<32 * 64).filter { index in
                let rgb = Array(pixels[index * 4..<index * 4 + 3]).map(Int.init)
                return (rgb.max() ?? 0) - (rgb.min() ?? 0) > 30
            }.count
            XCTAssertGreaterThan(colored, 20, "The actual preview did not render in the detail view")
            XCTAssertTrue(FileManager.default.fileExists(atPath: fixture.paths.data.path))
        }
    }

    private func withSession(_ body: (LibrarySession, BridgeFixture) async throws -> Void) async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        do {
            XCTAssertEqual(session.phase, .ready)
            try await body(session, fixture)
            await session.shutdown()
            try fixture.remove()
        } catch {
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    private func eventually(_ condition: () -> Bool) async throws {
        for _ in 0..<200 {
            if condition() { return }
            try await Task.sleep(for: .milliseconds(10))
        }
        XCTFail("Asynchronous preview did not reach the expected state")
    }
}
