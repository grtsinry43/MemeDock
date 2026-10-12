import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Thumbnails for a Telegram pack before anything is imported. The core
/// downloads them on demand, so the store keeps a small queue instead of
/// asking for a whole pack at once, and remembers what it already holds.
@MainActor
final class TelegramPreviewStore: ObservableObject {
    /// Two downloads at a time, matching the Linux queue: enough to fill a
    /// visible row without competing with the import that follows.
    private static let concurrency = 2

    @Published private(set) var images: [String: NSImage] = [:]
    @Published private(set) var failed: Set<String> = []

    private let session: LibrarySession
    private var pack: TelegramPackHandle?
    private var inFlight: Set<String> = []
    private var waiting: [String] = []
    private var running = 0

    init(session: LibrarySession) {
        self.session = session
    }

    /// A new pack invalidates every image: the same sticker id in another pack
    /// is a different file.
    func use(pack: TelegramPackHandle?) {
        self.pack = pack
        waiting.removeAll()
        inFlight.removeAll()
        images.removeAll()
        failed.removeAll()
    }

    func image(for id: String) -> NSImage? {
        images[id]
    }

    func load(_ id: String) {
        guard pack != nil,
              images[id] == nil,
              !failed.contains(id),
              !inFlight.contains(id)
        else { return }
        inFlight.insert(id)
        waiting.append(id)
        pump()
    }

    private func pump() {
        while running < Self.concurrency, !waiting.isEmpty {
            let id = waiting.removeFirst()
            running += 1
            Task { [weak self] in await self?.request(id) }
        }
    }

    private func request(_ id: String) async {
        defer {
            running -= 1
            inFlight.remove(id)
            pump()
        }
        guard let pack, let library = session.library else {
            failed.insert(id)
            return
        }
        do {
            let path = try await library.telegramPreview(pack: pack, id: id).awaitResult()
            guard let image = NSImage(contentsOfFile: path) else {
                failed.insert(id)
                return
            }
            images[id] = image
        } catch {
            failed.insert(id)
        }
    }
}
