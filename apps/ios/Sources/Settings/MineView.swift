import SwiftUI

enum AppAppearance: String, CaseIterable, Identifiable {
    case system, light, dark
    var id: Self { self }
    var title: LocalizedStringKey {
        switch self {
        case .system: "System"
        case .light: "Light"
        case .dark: "Dark"
        }
    }
    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

struct MineView: View {
    let session: LibrarySession
    @AppStorage("appearance") private var appearance: AppAppearance = .system
    @Environment(\.locale) private var locale

    var body: some View {
        List {
            Section {
                if let totals = session.statistics {
                    ViewThatFits(in: .horizontal) {
                        HStack(alignment: .top, spacing: 24) {
                            statistic("Original Count", totals.knownAssets.formatted(), icon: "face.smiling")
                            statistic("Originals", bytes(totals.readyOriginalBytes), icon: "photo")
                            statistic(
                                "Cache", bytes(totals.thumbnailBytes + totals.temporaryShareBytes),
                                icon: "internaldrive")
                        }
                        VStack(alignment: .leading, spacing: 16) {
                            LabeledContent("Original Count", value: totals.knownAssets.formatted())
                            LabeledContent("Originals", value: bytes(totals.readyOriginalBytes))
                            LabeledContent("Cache", value: bytes(totals.thumbnailBytes + totals.temporaryShareBytes))
                        }
                    }
                    .padding(.vertical, 8)
                } else if session.queryError == nil {
                    ProgressView()
                }
            } footer: {
                Text("Cache includes thumbnails and temporary sharing files")
            }
            Section("Library") {
                if let sync = session.sync {
                    NavigationLink(value: MineDestination.sync) {
                        LabeledContent {
                            Text(sync.title).foregroundStyle(.secondary)
                        } label: {
                            Label("Sync", systemImage: "arrow.triangle.2.circlepath")
                        }
                    }
                }
                NavigationLink(value: MineDestination.trash) {
                    Label("Trash", systemImage: "trash")
                }
                NavigationLink(value: MineDestination.backup) {
                    Label("Backup & Restore", systemImage: "externaldrive.badge.timemachine")
                }
                NavigationLink(value: MineDestination.libraryInfo) {
                    Label("Library & Storage", systemImage: "externaldrive")
                }
                if let error = session.queryError {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") { Task { await session.refreshStatistics() } }
                }
            }
            outputSettings
            Section("General") {
                Picker(selection: $appearance) {
                    ForEach(AppAppearance.allCases) { value in Text(value.title).tag(value) }
                } label: {
                    Label("Appearance", systemImage: "circle.lefthalf.filled")
                }
                if let settings = URL(string: UIApplication.openSettingsURLString) {
                    Link(destination: settings) {
                        HStack {
                            Label("Language", systemImage: "globe")
                            Spacer()
                            Text(
                                locale.localizedString(forIdentifier: Bundle.main.preferredLocalizations.first ?? "en")
                                    ?? "English"
                            )
                            .foregroundStyle(.secondary)
                            Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                        }
                    }
                    .foregroundStyle(.primary)
                    .accessibilityHint("Choose the app language in Settings.")
                }
            }
            Section("About") {
                LabeledContent {
                    Text(version).foregroundStyle(.secondary)
                } label: {
                    Label("Version", systemImage: "info.circle")
                }
                if let source = URL(string: "https://github.com/grtsinry43/MemeDock") {
                    NavigationLink(value: MineDestination.licenses) {
                        Label("Open Source Licenses", systemImage: "doc.text")
                    }
                    Link(destination: source) {
                        Label("Source Code", systemImage: "chevron.left.forwardslash.chevron.right")
                    }
                }
            }
        }
        .navigationTitle("Mine")
        .refreshable { await session.refreshStatistics() }
    }

    private var version: String {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "—"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "—"
        return "\(version) (\(build))"
    }

    private func bytes(_ value: Int64) -> String {
        ByteCountFormatStyle(locale: locale).format(value)
    }

    private var outputSettings: some View {
        @Bindable var preferences = session.outputPreferences
        return Section {
            outputPicker("Copy Still Images", value: $preferences.copyStatic, animated: false)
            outputPicker("Copy Animations", value: $preferences.copyAnimated, animated: true)
            outputPicker("Share and Save Still Images", value: $preferences.shareStatic, animated: false)
            outputPicker("Share and Save Animations", value: $preferences.shareAnimated, animated: true)
        } header: {
            Text("Output Defaults")
        } footer: {
            Text(
                "Still images and animations have separate defaults. Temporary detail-page choices only affect that operation. Saving uses the sharing default."
            )
        }
    }
    private func outputPicker(_ title: LocalizedStringKey, value: Binding<OutputPreset>, animated: Bool) -> some View {
        Picker(selection: value) {
            ForEach(OutputPreset.applicable(animated: animated)) { Text($0.title).tag($0) }
        } label: {
            Text(title)
        }
    }

    private func statistic(_ title: LocalizedStringKey, _ value: String, icon: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: icon).font(.title3).foregroundStyle(.secondary)
            Text(value).font(.title2.weight(.semibold)).fixedSize()
            Text(title).font(.caption).foregroundStyle(.secondary).fixedSize()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}
