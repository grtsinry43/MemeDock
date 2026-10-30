import MemeDockBridge
import SwiftUI
import UniformTypeIdentifiers

struct BackupView: View {
    @Bindable var model: BackupModel
    @State private var choosing = false
    @State private var replacing = false
    @Environment(\.locale) private var locale
    var body: some View {
        List {
            Section {
                if model.phase == .creating || model.phase == .saving {
                    progress("Creating Backup…")
                } else {
                    Button("Create Backup", systemImage: "archivebox") { model.create() }.disabled(
                        model.isBusy || model.phase == .preview)
                }
            } header: {
                Text("Backup")
            } footer: {
                Text("Export stickers, collections, tags, and Trash to back up your library or move to another device.")
            }
            Section {
                if model.phase == .reading {
                    progress("Reading Backup…")
                } else if model.phase == .restoring {
                    Label("Restoring…", systemImage: "archivebox")
                } else {
                    Button("Choose Backup File", systemImage: "archivebox") { choosing = true }.disabled(
                        model.isBusy || model.phase == .preview)
                }
            } header: {
                Text("Restore")
            } footer: {
                Text("Choose a MemeDock backup to inspect, merge, or replace the current library.")
            }
            if model.phase == .preview, let summary = model.summary {
                Section("Backup Contents") {
                    LabeledContent(
                        "Stickers",
                        value: (summary.stickers - min(summary.stickers, summary.deletedStickers)).formatted())
                    LabeledContent("Trash", value: summary.deletedStickers.formatted())
                    LabeledContent("Collections", value: summary.collections.formatted())
                    LabeledContent("Tags", value: summary.tags.formatted())
                    LabeledContent(
                        "Data Size",
                        value: ByteCountFormatStyle(locale: locale).format(Int64(clamping: summary.originalBytes)))
                    Text("\(summary.added) new items; \(summary.preserved) existing items will be kept.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section {
                    Button("Merge Import") { model.restore(.merge) }
                    Button("Replace Current Library…", role: .destructive) { replacing = true }
                    Button("Cancel") { Task { await model.dismissPreview() } }
                } footer: {
                    Text("Only import missing content. Keep existing organization. Deleted stickers will stay deleted.")
                }
            }
            if let error = model.error { Text(error).foregroundStyle(.red) }
        }
        .navigationTitle("Backup & Restore")
        .navigationBarBackButtonHidden(model.phase == .restoring)
        .fileImporter(isPresented: $choosing, allowedContentTypes: [.zip, .data]) { result in
            switch result {
            case .success(let url): model.inspect(url)
            case .failure(let error): model.selectionFailed(error)
            }
        }
        .sheet(item: $model.delivery, onDismiss: { Task { await model.completeDelivery(saved: false) } }) {
            BackupSavePicker(delivery: $0, model: model)
        }
        .confirmationDialog("Replace the Current Library?", isPresented: $replacing, titleVisibility: .visible) {
            Button("Replace Library", role: .destructive) { model.restore(.replace) }
        } message: {
            Text(
                "All current stickers, collections, and tags will be replaced. A local safety snapshot is kept before replacement."
            )
        }
    }
    private func progress(_ text: LocalizedStringKey) -> some View {
        HStack {
            ProgressView()
            Text(text)
            Spacer()
            Button("Cancel") { model.cancel() }
        }
    }
}

struct BackupSavePicker: UIViewControllerRepresentable {
    let delivery: BackupDelivery
    let model: BackupModel
    func makeCoordinator() -> Coordinator { Coordinator(model: model) }
    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(
            forExporting: [URL(fileURLWithPath: delivery.file.path())], asCopy: true)
        picker.delegate = context.coordinator
        return picker
    }
    func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}
    @MainActor final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let model: BackupModel
        init(model: BackupModel) { self.model = model }
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            Task { await model.completeDelivery(saved: !urls.isEmpty) }
        }
        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
            Task { await model.completeDelivery(saved: false) }
        }
    }
}
