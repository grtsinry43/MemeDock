import MemeDockBridge
import SwiftUI

/// The formats a sticker leaves the app in, per way of handing it over.
struct OutputSettings: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var preferences: Preferences

    var body: some View {
        SettingsForm {
            Section {
                presetRow(.outputCopy, detail: .outputCopyHint, selection: $preferences.copyPreset, animated: $preferences.copyAnimatedPreset)
                presetRow(.outputShare, detail: .outputShareHint, selection: $preferences.sharePreset, animated: $preferences.shareAnimatedPreset)
                presetRow(.outputDrag, detail: .outputDragHint, selection: $preferences.dragPreset, animated: $preferences.dragAnimatedPreset)
            } header: {
                SettingsHeader(.outputDefaults)
            } footer: {
                SettingsFooter(.outputFooter)
            }
        }
    }

    /// The menu cannot say what each choice does, so the chosen one is
    /// described right under it.
    private func presetRow(
        _ title: L10nKey,
        detail: L10nKey,
        selection: Binding<ExportPreset>, animated: Binding<ExportPreset>
    ) -> some View {
        LabeledContent {
            VStack(alignment: .trailing, spacing: 4) {
                HStack(spacing: 12) {
                    Picker(L10n.text(.outputStatic), selection: selection) {
                        ForEach(StickerExport.presets(animated: false), id: \.self) { preset in
                            Text(StickerExport.label(preset)).tag(preset)
                        }
                    }
                    .pickerStyle(.menu)
                    .fixedSize()
                    Picker(L10n.text(.outputAnimated), selection: animated) {
                        ForEach(StickerExport.presets(animated: true), id: \.self) { preset in
                            Text(StickerExport.label(preset)).tag(preset)
                        }
                    }
                    .pickerStyle(.menu)
                    .fixedSize()
                }
                Text(selection.wrappedValue == animated.wrappedValue ? StickerExport.hint(selection.wrappedValue) : StickerExport.hint(selection.wrappedValue) + " · " + StickerExport.hint(animated.wrappedValue))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .multilineTextAlignment(.trailing)
            }
        } label: {
            SettingsLabel(title, detail: detail)
        }
    }
}
