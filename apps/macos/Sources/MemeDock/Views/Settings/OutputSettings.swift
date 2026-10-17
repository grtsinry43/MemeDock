import MemeDockBridge
import SwiftUI

/// The formats a sticker leaves the app in, per way of handing it over.
struct OutputSettings: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var preferences: Preferences

    var body: some View {
        SettingsForm {
            Section {
                presetRow(.outputCopy, detail: .outputCopyHint, selection: $preferences.copyPreset)
                presetRow(.outputShare, detail: .outputShareHint, selection: $preferences.sharePreset)
                presetRow(.outputDrag, detail: .outputDragHint, selection: $preferences.dragPreset)
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
        selection: Binding<ExportPreset>
    ) -> some View {
        LabeledContent {
            VStack(alignment: .trailing, spacing: 4) {
                Picker(L10n.text(title), selection: selection) {
                    ForEach(StickerExport.presets, id: \.self) { preset in
                        Text(StickerExport.label(preset)).tag(preset)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
                .fixedSize()
                Text(StickerExport.hint(selection.wrappedValue))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .multilineTextAlignment(.trailing)
            }
        } label: {
            SettingsLabel(title, detail: detail)
        }
    }
}
