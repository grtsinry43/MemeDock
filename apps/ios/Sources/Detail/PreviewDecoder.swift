import Foundation
import ImageIO
import MemeDockBridge
import UniformTypeIdentifiers

enum PreviewError: LocalizedError {
    case invalidImage, resourceLimit, changedOriginal

    var errorDescription: String? {
        switch self {
        case .invalidImage: String(localized: "This image couldn’t be decoded for preview.")
        case .resourceLimit: String(localized: "This image exceeds the preview resource limits. The original is unchanged.")
        case .changedOriginal: String(localized: "The original image is missing or has changed. Try loading it again.")
        }
    }
}

struct PreviewDescription: Sendable {
    let frameCount: Int
    let delays: [Double]
    /// nil means unlimited plays. ImageIO normalizes GIF repetitions to total plays.
    let totalPlays: Int?
}

/// One source and one requested frame per detail view. All ImageIO work is actor-isolated.
/// Tokens prevent a cancelled reader or delayed cleanup from touching a replacement source.
actor PreviewDecoder {
    private var source: CGImageSource?
    private var token: UUID?
    private var pixels = 1
    private var count = 0

    func open(
        path: String, asset: AssetMetadata, limits: ResourceConfiguration,
        pixels: Int, token: UUID
    ) throws -> PreviewDescription {
        try Task.checkCancellation()
        source = nil
        self.token = nil
        count = 0
        let url = URL(fileURLWithPath: path)
        let file = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        guard file.isRegularFile == true, let size = file.fileSize,
              Int64(size) == asset.byteSize else { throw PreviewError.changedOriginal }
        guard UInt64(size) <= limits.maxFileBytes else { throw PreviewError.resourceLimit }
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [
            kCGImageSourceShouldCache: false,
        ] as CFDictionary), CGImageSourceGetStatus(source) == .statusComplete,
              let type = CGImageSourceGetType(source) else { throw PreviewError.invalidImage }
        let types = ["image/png": UTType.png.identifier, "image/jpeg": UTType.jpeg.identifier,
                     "image/gif": UTType.gif.identifier, "image/webp": UTType.webP.identifier]
        guard types[asset.mime] == type as String else { throw PreviewError.changedOriginal }
        let frameCount = CGImageSourceGetCount(source)
        // Bound metadata scanning as well as pixels; no array of decoded animation frames.
        guard frameCount > 0, frameCount <= 10_000 else { throw PreviewError.resourceLimit }
        var delays: [Double] = []
        for index in 0..<frameCount {
            try Task.checkCancellation()
            guard let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any],
                  let width = properties[kCGImagePropertyPixelWidth] as? NSNumber,
                  let height = properties[kCGImagePropertyPixelHeight] as? NSNumber else { throw PreviewError.invalidImage }
            let w = width.uint64Value
            let h = height.uint64Value
            guard w > 0, h > 0, w <= UInt64(asset.width), h <= UInt64(asset.height),
                  w <= limits.maxFramePixels / h,
                  // Match core's allowance for animation composition/orientation canvases.
                  w <= limits.maxDecodeBytes / 16 / h else { throw PreviewError.resourceLimit }
            delays.append(Self.delay(properties))
        }
        let properties = CGImageSourceCopyProperties(source, nil) as? [CFString: Any] ?? [:]
        let gif = properties[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        let png = properties[kCGImagePropertyPNGDictionary] as? [CFString: Any]
        let webp = properties[kCGImagePropertyWebPDictionary] as? [CFString: Any]
        let totalPlays: Int?
        if let repeats = gif?[kCGImagePropertyGIFLoopCount] as? NSNumber {
            totalPlays = repeats.intValue == 0 ? nil : max(1, repeats.intValue)
        } else if let plays = (png?[kCGImagePropertyAPNGLoopCount] ?? webp?[kCGImagePropertyWebPLoopCount]) as? NSNumber {
            totalPlays = plays.intValue == 0 ? nil : max(1, plays.intValue)
        } else {
            totalPlays = 1
        }
        self.source = source
        self.token = token
        self.pixels = max(1, min(pixels, 1280))
        count = frameCount
        return PreviewDescription(frameCount: frameCount, delays: delays, totalPlays: totalPlays)
    }

    func frame(at index: Int, token: UUID) throws -> CGImage {
        try Task.checkCancellation()
        guard self.token == token, let source else { throw CancellationError() }
        guard index >= 0, index < count,
              let image = CGImageSourceCreateThumbnailAtIndex(source, index, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: pixels,
                kCGImageSourceShouldCacheImmediately: true,
              ] as CFDictionary) else { throw PreviewError.invalidImage }
        CGImageSourceRemoveCacheAtIndex(source, index)
        try Task.checkCancellation()
        return image
    }

    func close(token: UUID) {
        guard self.token == token else { return }
        source = nil
        self.token = nil
        count = 0
    }

    private static func delay(_ properties: [CFString: Any]) -> Double {
        let gif = properties[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        let png = properties[kCGImagePropertyPNGDictionary] as? [CFString: Any]
        let webp = properties[kCGImagePropertyWebPDictionary] as? [CFString: Any]
        let number = gif?[kCGImagePropertyGIFUnclampedDelayTime]
            ?? png?[kCGImagePropertyAPNGUnclampedDelayTime]
            ?? webp?[kCGImagePropertyWebPUnclampedDelayTime]
            ?? gif?[kCGImagePropertyGIFDelayTime]
            ?? png?[kCGImagePropertyAPNGDelayTime]
            ?? webp?[kCGImagePropertyWebPDelayTime]
        let delay = (number as? NSNumber)?.doubleValue ?? 0.1
        return delay.isFinite && delay > 0 && delay <= 65_535 ? delay : 0.1
    }
}
