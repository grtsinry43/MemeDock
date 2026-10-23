import SwiftUI

struct ImportStatusView: View {
    let coordinator: ImportCoordinator
    let showProblems: () -> Void
    var showImported: ((String) -> Void)?

    var body: some View {
        if coordinator.isRunning {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(coordinator.isStopping ? "Stopping…" : "Importing \(coordinator.completed) of \(coordinator.items.count)")
                        .font(.subheadline)
                    Spacer()
                    Button("Cancel", action: coordinator.cancel)
                        .disabled(coordinator.isStopping)
                }
                ProgressView(value: Double(coordinator.completed), total: Double(coordinator.items.count))
                if let item = coordinator.items.first(where: { !$0.status.isFinished }) {
                    Text(item.name)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            .accessibilityElement(children: .contain)
        } else if !coordinator.items.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("\(coordinator.added) added, \(coordinator.reused) already in library")
                    .font(.subheadline)
                HStack {
                    if let showImported, let item = coordinator.items.last(where: { $0.status == .created || $0.status == .reused }),
                       let id = item.stickerID {
                        Button("View Imported") { showImported(id) }
                    }
                    if coordinator.hasProblems {
                        Button("Review Problems", action: showProblems)
                    }
                    Spacer()
                    Button("Done", action: coordinator.dismissReport)
                }
            }
        }
    }
}
