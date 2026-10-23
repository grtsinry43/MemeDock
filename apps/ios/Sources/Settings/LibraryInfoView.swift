import SwiftUI

struct LibraryInfoView: View {
    let session: LibrarySession

    var body: some View {
        List {
            if let identity = session.identity {
                Section("Identity") {
                    LabeledContent("Library ID", value: identity.libraryId)
                    LabeledContent("Device ID", value: identity.deviceId)
                }
                .textSelection(.enabled)
            }
            Section("Storage") {
                if let totals = session.statistics {
                    LabeledContent("Known Assets", value: totals.knownAssets.formatted())
                    bytes("Known Original Size", totals.knownOriginalBytes)
                    bytes("Originals on Device", totals.readyOriginalBytes)
                    bytes("Thumbnails", totals.thumbnailBytes)
                    bytes("Temporary Share Files", totals.temporaryShareBytes)
                } else if session.queryError == nil {
                    ProgressView()
                }
                if let error = session.queryError {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") { Task { await session.refreshStatistics() } }
                }
            }
        }
        .navigationTitle("Library Information")
        .navigationBarTitleDisplayMode(.inline)
        .refreshable { await session.refreshStatistics() }
    }

    private func bytes(_ title: LocalizedStringKey, _ count: Int64) -> some View {
        LabeledContent(title, value: ByteCountFormatter.string(fromByteCount: count, countStyle: .file))
    }
}
