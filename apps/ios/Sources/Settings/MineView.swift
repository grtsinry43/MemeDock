import SwiftUI

struct MineView: View {
    let session: LibrarySession

    var body: some View {
        List {
            Section("Library") {
                NavigationLink(value: MineDestination.libraryInfo) {
                    Label("Library Information", systemImage: "externaldrive")
                }
                if let totals = session.statistics {
                    LabeledContent("Known Assets", value: totals.knownAssets.formatted())
                    LabeledContent("Originals on Device", value: ByteCountFormatter.string(fromByteCount: totals.readyOriginalBytes, countStyle: .file))
                }
                if let error = session.queryError {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") { Task { await session.refreshStatistics() } }
                }
            }
            Section("About") {
                LabeledContent("Version", value: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "—")
            }
        }
        .navigationTitle("Mine")
        .refreshable { await session.refreshStatistics() }
    }
}
