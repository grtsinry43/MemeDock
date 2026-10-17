import Foundation
import MemeDockBridge

/// Platform-side mirror of the domain preset table. The core validates every
/// option it receives, so these values must stay identical to the Rust
/// defaults in `memedock_domain::export`.
enum StickerExport {
    struct Resolution {
        let preset: ExportPreset
        let animation: AnimationPolicy
    }

    static func presets(animated: Bool) -> [ExportPreset] {
        animated ? [.bestCompatibility, .original] : [.bestCompatibility, .original, .whiteBackground, .smallJpeg]
    }

    /// The generated enum carries no raw value, so the stored defaults use
    /// these names. They are persisted and must not change.
    static func storageKey(_ preset: ExportPreset) -> String {
        switch preset {
        case .original: return "original"
        case .compatiblePng: return "compatiblePng"
        case .whiteBackground: return "whiteBackground"
        case .smallJpeg: return "smallJpeg"
        case .compatibleGif: return "compatibleGif"
        case .bestCompatibility: return "bestCompatibility"
        }
    }

    static func preset(storageKey: String?, animated: Bool) -> ExportPreset? {
        if storageKey == "compatiblePng" || storageKey == "compatibleGif" { return .bestCompatibility }
        return presets(animated: animated).first { self.storageKey($0) == storageKey }
    }

    static func effectiveOverride(_ preset: ExportPreset?, animated: Bool) -> ExportPreset? {
        guard let preset, presets(animated: animated).contains(preset) else { return nil }
        return preset
    }

    /// The extension a Quick Look copy needs: the stored original has none.
    static func fileExtension(forMime mime: String) -> String? {
        switch mime {
        case "image/png": return "png"
        case "image/jpeg": return "jpg"
        case "image/gif": return "gif"
        case "image/webp": return "webp"
        default: return nil
        }
    }

    static func options(_ preset: ExportPreset, animation: AnimationPolicy) -> ExportOptions {
        ExportOptions(
            preset: preset,
            maxEdge: maxEdge(preset),
            backgroundRgba: background(preset),
            stripMetadata: preset != .original,
            animation: preset == .original ? .preserve : animation
        )
    }

    /// GIF preserves animation; the static presets keep animated originals
    /// untouched until the user explicitly asks for the first frame.
    static func resolve(preset: ExportPreset, animated: Bool, firstFrame: Bool) -> Resolution {
        if firstFrame {
            return Resolution(preset: .compatiblePng, animation: .firstFrame)
        }
        if preset == .bestCompatibility { return Resolution(preset: preset, animation: .preserve) }
        if preset == .original || (animated && !firstFrame) {
            return Resolution(preset: .original, animation: .preserve)
        }
        return Resolution(preset: preset, animation: .preserve)
    }

    static func label(_ preset: ExportPreset) -> String {
        switch preset {
        case .original: return L10n.text(.exportOriginal)
        case .compatiblePng: return L10n.text(.exportCompatiblePng)
        case .whiteBackground: return L10n.text(.exportWhiteBackground)
        case .smallJpeg: return L10n.text(.exportSmallJpeg)
        case .compatibleGif: return L10n.text(.exportCompatibleGif)
        case .bestCompatibility: return L10n.text(.exportBestCompatibility)
        }
    }

    static func hint(_ preset: ExportPreset) -> String {
        switch preset {
        case .original: return L10n.text(.exportOriginalHint)
        case .compatiblePng: return L10n.text(.exportCompatiblePngHint)
        case .whiteBackground: return L10n.text(.exportWhiteBackgroundHint)
        case .smallJpeg: return L10n.text(.exportSmallJpegHint)
        case .compatibleGif: return L10n.text(.exportCompatibleGifHint)
        case .bestCompatibility: return L10n.text(.exportBestCompatibilityHint)
        }
    }

    private static func maxEdge(_ preset: ExportPreset) -> UInt32? {
        switch preset {
        case .original, .whiteBackground, .bestCompatibility: return nil
        case .compatiblePng: return 1024
        case .smallJpeg, .compatibleGif: return 512
        }
    }

    private static func background(_ preset: ExportPreset) -> UInt32? {
        switch preset {
        case .whiteBackground, .smallJpeg: return 0xFFFF_FFFF
        case .original, .compatiblePng, .compatibleGif, .bestCompatibility: return nil
        }
    }
}
