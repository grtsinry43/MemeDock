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

    static func options(_ preset: ExportPreset, animation: AnimationPolicy) -> ExportOptions {
        ExportOptions(
            preset: preset,
            maxEdge: maxEdge(preset),
            backgroundRgba: background(preset),
            stripMetadata: preset != .original,
            animation: preset == .original ? .preserve : animation
        )
    }

    /// An animated original can only be handed over untouched, and asking for
    /// the first frame flattens it. Everything else keeps the animation.
    static func resolve(preset: ExportPreset, animated: Bool, firstFrame: Bool) -> Resolution {
        if preset == .original || (animated && !firstFrame) {
            return Resolution(preset: .original, animation: .preserve)
        }
        return Resolution(preset: preset, animation: firstFrame ? .firstFrame : .preserve)
    }

    static func label(_ preset: ExportPreset) -> String {
        switch preset {
        case .original: return L10n.text(.exportOriginal)
        case .compatiblePng: return L10n.text(.exportCompatiblePng)
        case .whiteBackground: return L10n.text(.exportWhiteBackground)
        case .smallJpeg: return L10n.text(.exportSmallJpeg)
        }
    }

    static func hint(_ preset: ExportPreset) -> String {
        switch preset {
        case .original: return L10n.text(.exportOriginalHint)
        case .compatiblePng: return L10n.text(.exportCompatiblePngHint)
        case .whiteBackground: return L10n.text(.exportWhiteBackgroundHint)
        case .smallJpeg: return L10n.text(.exportSmallJpegHint)
        }
    }

    private static func maxEdge(_ preset: ExportPreset) -> UInt32? {
        switch preset {
        case .original, .whiteBackground: return nil
        case .compatiblePng: return 1024
        case .smallJpeg: return 512
        }
    }

    private static func background(_ preset: ExportPreset) -> UInt32? {
        switch preset {
        case .whiteBackground, .smallJpeg: return 0xFFFF_FFFF
        case .original, .compatiblePng: return nil
        }
    }
}
