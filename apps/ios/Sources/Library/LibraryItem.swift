import Foundation
import MemeDockBridge

struct LibraryItem: Identifiable, Sendable {
    let sticker: StickerMetadata
    var resource: StickerResource?
    var thumbnailRevision: UInt64 = 0

    var id: String { sticker.id }
}

enum LibraryFilter: Hashable {
    case recentlyUsed, all, starred, collection(String)

    var sort: StickerSort {
        switch self {
        case .recentlyUsed: .lastUsed
        case .collection: .collectionOrder
        case .all, .starred: .recent
        }
    }
}

struct ThumbnailKey: Hashable, Sendable {
    let id: String
    let generation: Int64
    let path: String?
    let revision: UInt64
    let pixels: Int

    init(_ item: LibraryItem, pixels: Int) {
        id = item.id
        generation = item.sticker.lifecycle.generation
        path = item.resource?.thumbnailPath
        revision = item.thumbnailRevision
        self.pixels = max(1, min(pixels, 512))
    }

    var cacheKey: NSString {
        "\(id):\(generation):\(revision):\(pixels):\(path ?? "")" as NSString
    }
}
