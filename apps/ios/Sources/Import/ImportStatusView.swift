import SwiftUI

/// Import work that is running or waiting for the user; a clean finish is reported by a transient message instead.
struct ImportStatusView: View {
    let coordinator: ImportCoordinator
    let showProblems: () -> Void

    var body: some View {
        Group {
            if coordinator.isRunning {
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text(
                            coordinator.isStopping
                                ? "Stopping…" : "Importing \(coordinator.completed) of \(coordinator.items.count)"
                        )
                        .font(.subheadline).monospacedDigit()
                        .contentTransition(.numericText())
                        Spacer()
                        Button("Stop", action: coordinator.cancel)
                            .font(.subheadline)
                            .disabled(coordinator.isStopping)
                    }
                    ProgressView(value: Double(coordinator.completed), total: Double(max(1, coordinator.items.count)))
                }
                .padding(.horizontal, 16).padding(.vertical, 12)
                .accessibilityElement(children: .contain)
            } else if coordinator.hasProblems {
                HStack(spacing: 12) {
                    Button(action: showProblems) {
                        HStack(spacing: 12) {
                            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                                .accessibilityHidden(true)
                            Text(problemTitle).font(.subheadline).foregroundStyle(.primary)
                            Spacer(minLength: 0)
                            Image(systemName: "chevron.right").font(.footnote.weight(.semibold))
                                .foregroundStyle(.tertiary).accessibilityHidden(true)
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    Button("Dismiss", systemImage: "xmark", action: coordinator.dismissReport)
                        .labelStyle(.iconOnly).font(.footnote.weight(.semibold)).foregroundStyle(.secondary)
                        .frame(minWidth: 44, minHeight: 44)
                }
                .padding(.leading, 16).padding(.trailing, 4)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(uiColor: .secondarySystemGroupedBackground), in: .rect(cornerRadius: 12))
        .animation(.default, value: coordinator.completed)
    }

    private var problemTitle: String {
        let count = coordinator.problems.count
        return count > 0
            ? String(localized: "\(count) stickers failed to import") : String(localized: "Temporary File Cleanup")
    }
}
