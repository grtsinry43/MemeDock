import AppKit
import Combine
import Foundation

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
@MainActor
final class Preferences: ObservableObject {
    static let shared = Preferences()

    private enum Key {
        static let appearance = "appearance"
        static let language = "language"
    }

    private let defaults: UserDefaults
    @Published var appearance: AppearanceMode {
        didSet { persist() }
    }
    @Published var language: LanguageMode {
        didSet { persist() }
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        self.appearance = AppearanceMode(rawValue: defaults.string(forKey: Key.appearance) ?? "") ?? .system
        self.language = LanguageMode(rawValue: defaults.string(forKey: Key.language) ?? "") ?? .system
    }

    private func persist() {
        defaults.set(appearance.rawValue, forKey: Key.appearance)
        defaults.set(language.rawValue, forKey: Key.language)
    }
}
