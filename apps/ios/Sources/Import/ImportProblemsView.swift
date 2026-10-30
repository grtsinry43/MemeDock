import SwiftUI

struct ImportProblemsView: View {
    let coordinator: ImportCoordinator
    var viewTrash: () -> Void = {}
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                ForEach(coordinator.problems) { item in
                    HStack(spacing: 12) {
                        if let preview = item.preview {
                            Image(decorative: preview, scale: 1).resizable().scaledToFit().frame(width: 56, height: 56)
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            Text(item.name)
                            Text(message(item.status))
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                if coordinator.problems.contains(where: { $0.status != .restoreRequired }) {
                    Button("Retry Failed Imports") {
                        coordinator.retryProblems()
                        dismiss()
                    }.disabled(coordinator.isRunning)
                }
                if coordinator.problems.contains(where: { $0.status == .restoreRequired }) {
                    Button("View Trash") {
                        dismiss()
                        viewTrash()
                    }
                }
                if !coordinator.cleanupErrors.isEmpty {
                    Section("Temporary File Cleanup") {
                        ForEach(coordinator.cleanupErrors) { issue in
                            Text(issue.message)
                        }
                    }
                }
            }
            .navigationTitle("Import Problems")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private func message(_ status: ImportItemStatus) -> String {
        switch status {
        case .restoreRequired: String(localized: "This sticker is in the trash. Restore it before importing again.")
        case .cancelled: String(localized: "Import cancelled.")
        case .failed(let message): message
        default: ""
        }
    }
}
