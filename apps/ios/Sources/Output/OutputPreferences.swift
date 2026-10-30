import MemeDockBridge
import Observation
import SwiftUI

enum OutputPreset: String, CaseIterable, Identifiable {
    case best, original, white, small
    var id: Self { self }
    var title: LocalizedStringKey {
        switch self {
        case .best: "Best Compatibility"
        case .original: "Original"
        case .white: "White Background PNG"
        case .small: "Small Image"
        }
    }
    var hint: LocalizedStringKey {
        switch self {
        case .best: "Transparent PNG for still images; GIF for animations."
        case .original: "Keep the original file, transparency, and animation."
        case .white: "Fill a white background and keep the original size."
        case .small: "Resize to 512px with a white background."
        }
    }
    var bridge: ExportPreset {
        switch self {
        case .best: .bestCompatibility
        case .original: .original
        case .white: .whiteBackground
        case .small: .smallJpeg
        }
    }
    static func applicable(animated: Bool) -> [Self] { animated ? [.best, .original] : allCases }
}

enum OutputAction { case share, copy, photos, files }

@MainActor @Observable
final class OutputPreferences {
    var copyStatic: OutputPreset { didSet { persist() } }
    var copyAnimated: OutputPreset { didSet { persist() } }
    var shareStatic: OutputPreset { didSet { persist() } }
    var shareAnimated: OutputPreset { didSet { persist() } }
    init() {
        func value(_ key: String, animated: Bool) -> OutputPreset {
            guard let raw = UserDefaults.standard.string(forKey: key), let value = OutputPreset(rawValue: raw),
                OutputPreset.applicable(animated: animated).contains(value)
            else { return .best }
            return value
        }
        copyStatic = value("output.copy.static", animated: false)
        copyAnimated = value("output.copy.animated", animated: true)
        shareStatic = value("output.share.static", animated: false)
        shareAnimated = value("output.share.animated", animated: true)
    }
    func choice(_ action: OutputAction, animated: Bool) -> OutputPreset {
        action == .copy ? (animated ? copyAnimated : copyStatic) : (animated ? shareAnimated : shareStatic)
    }
    private func persist() {
        for (key, value) in [
            ("output.copy.static", copyStatic), ("output.copy.animated", copyAnimated),
            ("output.share.static", shareStatic), ("output.share.animated", shareAnimated),
        ] {
            UserDefaults.standard.set(value.rawValue, forKey: key)
        }
    }
}

struct ExportPresetPicker: View {
    @Binding var selection: OutputPreset?
    let animated: Bool
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                choice("Use Default Settings", checked: selection == nil) { selection = nil }
                ForEach(OutputPreset.applicable(animated: animated)) { preset in
                    Button {
                        selection = preset
                        dismiss()
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(preset.title).foregroundStyle(.primary)
                                Text(preset.hint).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if selection == preset { Image(systemName: "checkmark") }
                        }
                    }
                }
            }
            .navigationTitle("Send Format").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
        }.presentationDetents([.medium, .large])
    }
    private func choice(_ title: LocalizedStringKey, checked: Bool, action: @escaping () -> Void) -> some View {
        Button {
            action()
            dismiss()
        } label: {
            HStack {
                Text(title)
                Spacer()
                if checked { Image(systemName: "checkmark") }
            }
        }
    }
}
