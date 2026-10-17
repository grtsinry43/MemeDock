import SwiftUI

/// What the library occupies, what can be given back, and where it lives.
struct StorageSettings: View {
    @ObservedObject var session: LibrarySession
    let status: StatusCenter

    @State private var cleaning = false

    var body: some View {
        SettingsForm {
            spaceSection
            cleanupSection
            locationSection
        }
        .task { await session.refreshStatistics() }
    }

    private var spaceSection: some View {
        Section {
            if let statistics = session.statistics {
                LabeledContent(L10n.text(.originalCount)) {
                    SettingsFigure(text: "\(statistics.knownAssets)", value: Double(statistics.knownAssets))
                }
                LabeledContent(L10n.text(.originalBytes)) {
                    SettingsFigure(
                        text: Format.size(statistics.readyOriginalBytes),
                        value: Double(statistics.readyOriginalBytes)
                    )
                }
                // A thumbnail and a prepared share are both rebuildable cache, so
                // they are one line, the way Linux counts them.
                let cache = statistics.thumbnailBytes + statistics.temporaryShareBytes
                LabeledContent(L10n.text(.cacheBytes)) {
                    SettingsFigure(text: Format.size(cache), value: Double(cache))
                }
            } else {
                SettingsProgress(.loading)
            }
        } header: {
            SettingsHeader(.space)
        }
    }

    private var cleanupSection: some View {
        Section {
            LabeledContent(L10n.text(.cleanExports)) {
                if cleaning {
                    ProgressView().controlSize(.small)
                } else {
                    Button(L10n.text(.clean)) { clean() }
                }
            }
        } header: {
            SettingsHeader(.cleanUp)
        } footer: {
            SettingsFooter(.cleanExportsHint)
        }
    }

    private var locationSection: some View {
        Section {
            SettingsPathRow(title: L10n.text(.dataDirectory), path: session.paths.data.path)
            SettingsPathRow(title: L10n.text(.cacheDirectory), path: session.paths.cache.path)
            SettingsPathRow(title: L10n.text(.exportDirectory), path: session.paths.export.path)
        } header: {
            SettingsHeader(.locations)
        }
    }

    private func clean() {
        guard !cleaning else { return }
        cleaning = true
        Task {
            defer { cleaning = false }
            switch await session.cleanExportArtifacts() {
            case .success:
                status.show(.exportsCleaned)
                await session.refreshStatistics()
            case let .failure(error):
                status.show(error)
            }
        }
    }
}
