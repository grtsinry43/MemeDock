import Foundation
import MemeDockBridge

/// One cell in the library grid. The asset and thumbnail state come from the
/// core; an item without resources yet keeps `asset` empty instead of
/// inventing dimensions the library never reported.
struct StickerItem: Identifiable, Equatable {
    let id: String
    var sticker: StickerMetadata
    var asset: AssetMetadata?
    var thumbnailStatus: ThumbnailStatus
    var thumbnailPath: String?
    var lastError: String?

    init(sticker: StickerMetadata, resource: StickerResource?) {
        self.id = sticker.id
        self.sticker = sticker
        self.asset = resource?.asset
        self.thumbnailStatus = resource?.thumbnailStatus ?? .missing
        self.thumbnailPath = resource?.thumbnailPath
        self.lastError = resource?.lastError
    }

    var animated: Bool { asset?.animated ?? false }
    var mime: String { asset?.mime ?? "" }
    var byteSize: Int64 { asset?.byteSize ?? 0 }

    var isDeleted: Bool { sticker.lifecycle.deletedAt != nil }

    var pixelSize: String? {
        guard let asset, asset.width > 0, asset.height > 0 else { return nil }
        return "\(asset.width) × \(asset.height)"
    }

    var reference: EntityReference {
        EntityReference(id: id, generation: sticker.lifecycle.generation)
    }

    mutating func apply(detail: StickerDetail) {
        sticker = detail.sticker
        asset = detail.asset
    }

    mutating func apply(resource: StickerResource) {
        asset = resource.asset
        thumbnailStatus = resource.thumbnailStatus
        thumbnailPath = resource.thumbnailPath
        lastError = resource.lastError
    }
}
