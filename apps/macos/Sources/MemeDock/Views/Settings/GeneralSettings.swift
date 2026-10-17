import SwiftUI

/// How the app looks and which language it speaks.
struct GeneralSettings: View {
    @ObservedObject var preferences: Preferences

    var body: some View {
        SettingsForm {
            Section {
                // Three short choices read best side by side.
                Picker(L10n.text(.appearance), selection: $preferences.appearance) {
                    ForEach(AppearanceMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .fixedSize()
                Picker(L10n.text(.language), selection: $preferences.language) {
                    ForEach(LanguageMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.menu)
                .fixedSize()
            }
        }
    }
}
