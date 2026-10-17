import AppKit
import Combine
import Foundation
import MemeDockBridge

enum AppearanceMode: String, CaseIterable, Identifiable {
    case system
    case light
    case dark

    var id: String { rawValue }

    var label: String {
        switch self {
        case .system: return L10n.text(.system)
        case .light: return L10n.text(.light)
        case .dark: return L10n.text(.dark)
        }
    }

    var appearance: NSAppearance? {
        switch self {
        case .system: return nil
        case .light: return NSAppearance(named: .aqua)
        case .dark: return NSAppearance(named: .darkAqua)
        }
    }
}

enum LanguageMode: String, CaseIterable, Identifiable {
    case system
    case chinese
    case english

    var id: String { rawValue }

    var label: String {
        switch self {
        case .system: return L10n.text(.system)
        case .chinese: return L10n.text(.chinese)
        case .english: return L10n.text(.english)
        }
    }

    /// Resolved language for `system`, which follows the first preferred
    /// system language and falls back to English.
    var resolved: Language {
        switch self {
        case .chinese: return .chinese
        case .english: return .english
        case .system:
            let preferred = Locale.preferredLanguages.first ?? "en"
            return preferred.hasPrefix("zh") ? .chinese : .english
        }
    }
}

/// User choices that live outside the library database. They are stored in
/// user defaults, so they survive library deletion and stay per-user.
///
/// Every change is applied here as well as stored, so the appearance and the
/// language hold whether the settings window is opened next to the library or
/// on its own; views only need to observe this object to be re-rendered.
@MainActor
final class Preferences: ObservableObject {
    static let shared = Preferences()

    private enum Key {
        static let appearance = "appearance"
        static let language = "language"
        static let copyPreset = "copyPreset"
        static let sharePreset = "sharePreset"
        static let dragPreset = "dragPreset"
        static let tileSize = "tileSize"
    }

    private let defaults: UserDefaults
    @Published var appearance: AppearanceMode {
        didSet {
            persist()
            apply()
        }
    }
    @Published var language: LanguageMode {
        didSet {
            persist()
            apply()
        }
    }

    /// What copying an image or a file hands over unless the detail pane
    /// overrides it.
    @Published var copyPreset: ExportPreset {
        didSet { persist() }
    }
    @Published var copyAnimatedPreset: ExportPreset { didSet { persist() } }
    /// What sharing and saving hand over; both produce one file for somewhere
    /// else, so they share the choice.
    @Published var sharePreset: ExportPreset {
        didSet { persist() }
    }
    @Published var shareAnimatedPreset: ExportPreset { didSet { persist() } }
    /// What a sticker dragged into another app hands over. The detail pane's
    /// temporary choice never changes it.
    @Published var dragPreset: ExportPreset {
        didSet { persist() }
    }
    @Published var dragAnimatedPreset: ExportPreset { didSet { persist() } }

    /// How wide one grid tile aims to be, within `tileSizeRange`. A grid
    /// density is a display choice, so it survives relaunches.
    @Published var tileSize: CGFloat {
        didSet { persist() }
    }

    static var defaultTileSize: CGFloat { 112 }
    static let tileSizeRange: ClosedRange<CGFloat> = 80 ... 200
    private static let tileSizeStep: CGFloat = 16

    func largerStickers() {
        tileSize = min(Self.tileSizeRange.upperBound, tileSize + Self.tileSizeStep)
    }

    func smallerStickers() {
        tileSize = max(Self.tileSizeRange.lowerBound, tileSize - Self.tileSizeStep)
    }

    func resetStickerSize() {
        tileSize = Self.defaultTileSize
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        self.appearance = AppearanceMode(rawValue: defaults.string(forKey: Key.appearance) ?? "") ?? .system
        self.language = LanguageMode(rawValue: defaults.string(forKey: Key.language) ?? "") ?? .system
        OutputDefaultsV0_1_0ToV0_1_2.migrate(defaults)
        self.copyPreset = Self.loadOutput(defaults, Key.copyPreset, animated: false)
        self.copyAnimatedPreset = Self.loadOutput(defaults, Key.copyPreset, animated: true)
        self.sharePreset = Self.loadOutput(defaults, Key.sharePreset, animated: false)
        self.shareAnimatedPreset = Self.loadOutput(defaults, Key.sharePreset, animated: true)
        self.dragPreset = Self.loadOutput(defaults, Key.dragPreset, animated: false)
        self.dragAnimatedPreset = Self.loadOutput(defaults, Key.dragPreset, animated: true)
        let storedSize = defaults.object(forKey: Key.tileSize) as? Double
        self.tileSize = min(
            Self.tileSizeRange.upperBound,
            max(Self.tileSizeRange.lowerBound, storedSize.map { CGFloat($0) } ?? Self.defaultTileSize)
        )
    }

    /// Applies to the running app. The language is read at render time, so no
    /// relaunch is involved: the published change re-renders every window.
    func apply() {
        NSApp.appearance = appearance.appearance
        L10n.language = language.resolved
    }

    private func persist() {
        defaults.set(appearance.rawValue, forKey: Key.appearance)
        defaults.set(language.rawValue, forKey: Key.language)
        for (key, still, animated) in [(Key.copyPreset, copyPreset, copyAnimatedPreset), (Key.sharePreset, sharePreset, shareAnimatedPreset), (Key.dragPreset, dragPreset, dragAnimatedPreset)] {
            defaults.set(StickerExport.storageKey(still), forKey: Self.outputKey(key, animated: false))
            defaults.set(StickerExport.storageKey(animated), forKey: Self.outputKey(key, animated: true))
        }
        defaults.set(Double(tileSize), forKey: Key.tileSize)
    }

    private static func outputKey(_ legacyKey: String, animated: Bool) -> String {
        "\(legacyKey).\(animated ? "animated" : "static")"
    }

    private static func loadOutput(_ defaults: UserDefaults, _ key: String, animated: Bool) -> ExportPreset {
        StickerExport.preset(storageKey: defaults.string(forKey: outputKey(key, animated: animated)), animated: animated) ?? .bestCompatibility
    }

    /// One transition from the initial release's preference shape to 0.1.2.
    /// Each destination key is written independently, making interrupted and
    /// repeated migration safe. Legacy keys remain intact; no downgrade promise.
    @MainActor private enum OutputDefaultsV0_1_0ToV0_1_2 {
        static func migrate(_ defaults: UserDefaults) {
            for key in [Key.copyPreset, Key.sharePreset, Key.dragPreset] {
                let old = defaults.string(forKey: key)
                for animated in [false, true] {
                    let destination = outputKey(key, animated: animated)
                    guard defaults.object(forKey: destination) == nil else { continue }
                    let preset: ExportPreset
                    if old == "original" { preset = .original }
                    else if old == "whiteBackground" || old == "smallJpeg" {
                        preset = animated ? .original : (old == "whiteBackground" ? .whiteBackground : .smallJpeg)
                    } else { preset = .bestCompatibility }
                    defaults.set(StickerExport.storageKey(preset), forKey: destination)
                }
            }
        }
    }
}
