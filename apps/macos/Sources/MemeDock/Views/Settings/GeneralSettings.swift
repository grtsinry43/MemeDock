import SwiftUI
import KeyboardShortcuts
import ServiceManagement

/// How the app looks and which language it speaks.
struct GeneralSettings: View {
    @ObservedObject var preferences: Preferences
    @ObservedObject var integration: SystemIntegration

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
            Section {
                Toggle(L10n.text(.showDockIcon), isOn: $preferences.showDockIcon)
                Toggle(L10n.text(.launchAtLogin), isOn: Binding(
                    get: { integration.loginStatus == .enabled || integration.loginStatus == .requiresApproval },
                    set: integration.setLaunchAtLogin
                ))
                if integration.loginStatus == .requiresApproval {
                    Button(L10n.text(.approveLoginItem), action: integration.openLoginSettings)
                }
                Text(L10n.text(.residentHint)).font(.callout).foregroundStyle(.secondary)
            }
            Section {
                KeyboardShortcuts.Recorder(L10n.text(.quickPickerShortcut), name: .quickPicker) { _ in
                    integration.shortcutChanged()
                }
                .environment(\.locale, Locale(identifier: preferences.language.resolved == .chinese ? "zh-Hans" : "en"))
                Toggle(L10n.text(.anchorToInput), isOn: $preferences.anchorToInput)
                Text(L10n.text(.anchorToInputHint)).font(.callout).foregroundStyle(.secondary)
                Text(L10n.text(.pasteAccessHint)).font(.callout).foregroundStyle(.secondary)
                LabeledContent(L10n.text(.accessibilityAccess)) {
                    Text(L10n.text(integration.accessibilityGranted && integration.canPostEvents ? .accessibilityGranted : .accessibilityNotGranted))
                        .foregroundStyle(.secondary)
                }
                if !integration.accessibilityGranted || !integration.canPostEvents {
                    Button(L10n.text(.grantAccessibility), action: integration.openAccessibilitySettings)
                }
            }
        }
        .onAppear { integration.refresh() }
    }
}
