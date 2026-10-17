import MemeDockBridge
import SwiftUI

/// The whole library in and out of one file, and the database snapshot kept
/// beside it. Restoring previews what the archive holds and what it would do
/// before anything is written.
struct BackupSettings: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var session: LibrarySession
    @ObservedObject var coordinator: BackupCoordinator
    let status: StatusCenter

    @State private var checkpointing = false
    @State private var checkpoint: String?

    var body: some View {
        SettingsForm {
            backupSection
            restoreSection
            checkpointSection
        }
        // A preview holds a staged copy inside the library, so leaving the page
        // hands it back. A running restore owns it and finishes on its own.
        .onDisappear {
            guard coordinator.phase != .restoring else { return }
            Task { await coordinator.discardPreview() }
        }
    }

    private var backupSection: some View {
        Section {
            LabeledContent {
                Button(L10n.text(.backUp)) {
                    Task { await coordinator.createBackup() }
                }
                .disabled(coordinator.isBusy)
            } label: {
                SettingsLabel(.createBackup, detail: .backupHint)
            }
            if coordinator.phase == .creating {
                busyRow
            }
            if let savedPath = coordinator.savedPath {
                SettingsPathRow(title: L10n.text(.backupSavedTo), path: savedPath)
            }
        } header: {
            SettingsHeader(.backup)
        }
    }

    private var restoreSection: some View {
        Section {
            LabeledContent {
                Button(L10n.text(.chooseFile)) {
                    Task { await coordinator.chooseArchive() }
                }
                .disabled(coordinator.isBusy || coordinator.hasPreview)
            } label: {
                SettingsLabel(.restoreBackup, detail: .restoreHintShort)
            }
            if coordinator.phase == .inspecting || coordinator.phase == .restoring {
                busyRow
            }
            if let inspection = coordinator.inspection {
                preview(inspection)
            }
            if let message = coordinator.failure {
                Label(message, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(MemeDockPalette(scheme).error)
            } else if let restored = coordinator.restored {
                Label(resultText(restored), systemImage: "checkmark.circle")
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
            }
        }
    }

    @ViewBuilder
    private func preview(_ inspection: ArchiveSummary) -> some View {
        LabeledContent(L10n.text(.backupPreview)) {
            Text(L10n.text(.archiveContains, inspection.stickers, inspection.collections, inspection.tags))
                .monospacedDigit()
                .foregroundStyle(MemeDockPalette(scheme).secondary)
        }
        Picker(selection: $coordinator.mode) {
            Text(L10n.text(.merge)).tag(ArchiveRestoreMode.merge)
            Text(L10n.text(.replace)).tag(ArchiveRestoreMode.replace)
        } label: {
            SettingsLabel(
                L10n.text(.restoreMode),
                detail: L10n.text(coordinator.mode == .merge ? .backupMergeHint : .replaceHint)
            )
        }
        .pickerStyle(.segmented)
        .disabled(coordinator.isBusy)
        LabeledContent {
            HStack(spacing: 8) {
                Button(L10n.text(.discard)) {
                    Task { await coordinator.discardPreview() }
                }
                Button(L10n.text(.restore)) {
                    Task { await coordinator.restore() }
                }
                .keyboardShortcut(.defaultAction)
            }
            .disabled(coordinator.isBusy)
        } label: {
            if coordinator.mode == .merge {
                Text(L10n.text(.backupMergeSummary, inspection.added, inspection.preserved))
                    .monospacedDigit()
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
            }
        }
    }

    private var checkpointSection: some View {
        Section {
            LabeledContent {
                Button(L10n.text(.create)) { createCheckpoint() }
                    .disabled(checkpointing)
            } label: {
                SettingsLabel(.createCheckpoint, detail: .checkpointHint)
            }
            if checkpointing {
                SettingsProgress()
            }
            if let checkpoint {
                SettingsPathRow(title: L10n.text(.checkpointCreated), path: checkpoint)
            }
        }
    }

    private var busyRow: some View {
        LabeledContent {
            Button(L10n.text(.cancel)) { coordinator.cancel() }
        } label: {
            SettingsProgress()
        }
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

    private func createCheckpoint() {
        guard !checkpointing else { return }
        checkpointing = true
        Task {
            defer { checkpointing = false }
            switch await session.createCheckpoint() {
            case let .success(path):
                checkpoint = path
                status.show(.checkpointCreated)
            case let .failure(error):
                status.show(error)
            }
        }
    }
}
