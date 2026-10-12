import MemeDockBridge
import SwiftUI

/// Backup and restore share one sheet because they are the two ends of the same
/// file. Restoring shows what the archive holds and what it would do before
/// anything is written, and the two modes are chosen here.
struct BackupView: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var coordinator: BackupCoordinator

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 12) {
                Text(L10n.text(.backup))
                    .font(MemeDockTypography.sectionTitle)
                Text(L10n.text(.backupHint))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
                HStack(spacing: 12) {
                    Button(L10n.text(.createBackup)) {
                        Task { await coordinator.createBackup() }
                    }
                    Button(L10n.text(.restoreBackup)) {
                        Task { await coordinator.chooseArchive() }
                    }
                    .disabled(coordinator.hasPreview)
                }
                .disabled(coordinator.isBusy)

                if let savedPath = coordinator.savedPath {
                    HStack(spacing: 8) {
                        Image(systemName: "checkmark.circle")
                            .foregroundStyle(.green)
                        Text(savedPath)
                            .font(MemeDockTypography.body)
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .textSelection(.enabled)
                        Button(L10n.text(.revealInFinder)) { coordinator.revealSaved() }
                            .controlSize(.small)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)

            if let inspection = coordinator.inspection {
                Divider()
                preview(inspection)
            }

            Divider()
            footer
        }
        .frame(width: 580)
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).accent)
        .buttonStyle(MemeDockButtonStyle())
        .background(MemeDockPalette(scheme).canvas)
        // A preview holds a staged copy inside the library, so leaving the
        // sheet by any route has to hand it back.
        .onDisappear {
            Task { await coordinator.discardPreview() }
        }
    }

    private func preview(_ inspection: ArchiveSummary) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L10n.text(.backupPreview))
                .font(MemeDockTypography.rowTitle)
            Text(L10n.text(.archiveContains, inspection.stickers, inspection.collections, inspection.tags))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)

            Picker("", selection: $coordinator.mode) {
                Text(L10n.text(.merge)).tag(ArchiveRestoreMode.merge)
                Text(L10n.text(.replace)).tag(ArchiveRestoreMode.replace)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .disabled(coordinator.isBusy)

            Text(coordinator.mode == .merge ? L10n.text(.backupMergeHint) : L10n.text(.replaceHint))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            if coordinator.mode == .merge {
                Text(L10n.text(.backupMergeSummary, inspection.added, inspection.preserved))
                    .font(MemeDockTypography.body)
                    .monospacedDigit()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
    }

    private var footer: some View {
        HStack(spacing: 12) {
            if coordinator.isBusy {
                ProgressView()
                    .controlSize(.small)
                Text(L10n.text(.applying))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
            }

            if let message = coordinator.failure {
                Label(message, systemImage: "exclamationmark.triangle")
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            } else if let restored = coordinator.restored {
                Text(resultText(restored))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Spacer()

            if coordinator.isBusy {
                Button(L10n.text(.cancel)) { coordinator.cancel() }
            }
            Button(L10n.text(.close)) {
                Task { await coordinator.discardPreview() }
                dismiss()
            }
            Button(L10n.text(.restoreBackup)) {
                Task { await coordinator.restore() }
            }
            .keyboardShortcut(.defaultAction)
            .buttonStyle(MemeDockButtonStyle(kind: .primary))
            .disabled(!coordinator.hasPreview || coordinator.isBusy)
        }
        .padding(16)
        .background(MemeDockGlass())
    }

    /// A merge reports what it moved in; a replace leaves the library holding
    /// exactly what the archive held.
    private func resultText(_ restored: ArchiveSummary) -> String {
        switch coordinator.mode {
        case .merge:
            return L10n.text(.backupRestored) + " · " + L10n.text(.backupMergeSummary, restored.added, restored.preserved)
        case .replace:
            return L10n.text(.backupRestored) + " · "
                + L10n.text(.archiveContains, restored.stickers, restored.collections, restored.tags)
        }
    }
}
