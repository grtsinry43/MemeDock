import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Keeps decoded thumbnails for the grid. The core owns generation: the store
/// only asks for a thumbnail when the item reports it is missing, and reloads
/// when the reported path changes.
@MainActor
final class ThumbnailStore: ObservableObject {
    @Published private(set) var images: [String: NSImage] = [:]
    @Published private(set) var failed: Set<String> = []

    private let session: LibrarySession
    private var inFlight: Set<String> = []
    private var paths: [String: String] = [:]

    init(session: LibrarySession) {
        self.session = session
    }

    func image(for item: StickerItem) -> NSImage? {
        guard let path = item.thumbnailPath, paths[item.id] == path else { return nil }
        return images[item.id]
    }

    func load(_ item: StickerItem, priority: Priority = .visible) {
        if let path = paths[item.id], path != item.thumbnailPath {
            images[item.id] = nil
            paths[item.id] = nil
            failed.remove(item.id)
        }
        guard images[item.id] == nil, !inFlight.contains(item.id) else { return }
        switch item.thumbnailStatus {
        case .ready:
            guard let path = item.thumbnailPath else { return }
            decode(item.id, path: path)
        case .missing:
            generate(item.id, priority: priority)
        case .generating:
            break
        case .failed:
            failed.insert(item.id)
        }
    }

    /// Called when a thumbnail notification arrives; the reported path may have
    /// changed even when the status is unchanged.
    func invalidate(_ stickerId: String) {
        images[stickerId] = nil
        paths[stickerId] = nil
        failed.remove(stickerId)
    }

    func clear() {
        images.removeAll()
        paths.removeAll()
        failed.removeAll()
        inFlight.removeAll()
    }

    private func decode(_ stickerId: String, path: String) {
        guard let image = NSImage(contentsOfFile: path) else {
            failed.insert(stickerId)
            return
        }
        paths[stickerId] = path
        images[stickerId] = image
    }

    private func generate(_ stickerId: String, priority: Priority) {
        inFlight.insert(stickerId)
        Task { [weak self] in
            guard let self else { return }
            defer { self.inFlight.remove(stickerId) }
            do {
                let path = try await self.session.requestThumbnail(stickerId, priority: priority)
                self.decode(stickerId, path: path)
            } catch {
                self.failed.insert(stickerId)
            }
        }
    }
}
