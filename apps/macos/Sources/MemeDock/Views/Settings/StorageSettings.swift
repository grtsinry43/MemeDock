import MemeDockBridge
import SwiftUI

/// What the library occupies, what can be given back, where it lives, and how
/// to take a collection out of it. Export belongs on this page rather than
/// beside the backup: a backup is the whole library in a restorable file, an
/// export is finished images someone else can open.
struct StorageSettings: View {
    @ObservedObject var session: LibrarySession
    let status: StatusCenter

    @StateObject private var export: CollectionExport
    @State private var collections: [CollectionMetadata] = []
    /// nil means every collection.
    @State private var selection: String?
    @State private var cleaning = false

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
        _export = StateObject(wrappedValue: CollectionExport(session: session, status: status))
    }

    var body: some View {
        SettingsForm {
            spaceSection
            exportSection
            cleanupSection
            locationSection
        }
        .task {
            await session.refreshStatistics()
            await loadCollections()
        }
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

    /// The list is read when the page appears rather than subscribed to: a
    /// rename made in the library window shows up next time this page is
    /// opened, and picking a collection that has since gone reports itself.
    private func loadCollections() async {
        collections = (try? await session.collections()) ?? []
        if let selection, !collections.contains(where: { $0.id == selection }) {
            self.selection = nil
        }
    }

    private var exportSection: some View {
        Section {
            Picker(L10n.text(.collections), selection: $selection) {
                Text(L10n.text(.allCollections)).tag(String?.none)
                ForEach(collections, id: \.id) { collection in
                    Text(collection.name).tag(String?.some(collection.id))
                }
            }
            LabeledContent(L10n.text(.exportCollection)) {
                if export.isRunning {
                    ProgressView().controlSize(.small)
                } else {
                    Button(L10n.text(.export)) { run() }
                        .disabled(collections.isEmpty)
                }
            }
        } header: {
            SettingsHeader(.exportCollections)
        } footer: {
            SettingsFooter(.exportCollectionsHint)
        }
    }

    private func run() {
        guard let selection else {
            export.presentAll()
            return
        }
        guard let collection = collections.first(where: { $0.id == selection }) else {
            status.show(.errorNotFound)
            return
        }
        export.present(collection)
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
