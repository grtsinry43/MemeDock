import SwiftUI

struct ImportProblemsView: View {
    let coordinator: ImportCoordinator
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                ForEach(coordinator.problems) { item in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(item.name)
                        Text(message(item.status))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
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
