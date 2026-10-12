import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Whole-library backup and restore.
///
/// The core only ever reads and writes archives inside the library's own data
/// directory, so both directions pass through a private file this type copies
/// to or from; the save and open panels live here because they are the only
/// part that belongs to the platform.
@MainActor
final class BackupCoordinator: ObservableObject {
    enum Phase: Equatable {
        case idle
        case creating
        case inspecting
        case restoring
    }

    @Published var mode: ArchiveRestoreMode = .merge
    @Published private(set) var phase: Phase = .idle
    /// What the chosen archive holds and what a merge would do to it.
    @Published private(set) var inspection: ArchiveSummary?
    /// What the last restore actually did.
    @Published private(set) var restored: ArchiveSummary?
    @Published private(set) var failure: String?
    @Published private(set) var savedPath: String?

    private let session: LibrarySession
    private let status: StatusCenter
    private var prepared: PreparedArchiveHandle?
    private var cancelAction: (() -> Void)?

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
    }

    var isBusy: Bool {
        phase != .idle
    }

    var hasPreview: Bool {
        inspection != nil
    }

    // MARK: - Backup

    func createBackup() async {
        guard phase == .idle else { return }
        guard let library = session.library else {
            failure = describe(AppFailure(.errorClosed))
            return
        }
        failure = nil
        savedPath = nil
        restored = nil
        phase = .creating
        defer {
            phase = .idle
            cancelAction = nil
        }
        var backup: BackupFileHandle?
        do {
            let task = try library.createBackup()
            cancelAction = { _ = task.cancel() }
            let handle = try await task.awaitResult()
            backup = handle
            guard let url = Self.askDestination(name: handle.fileName()) else {
                // Cancelling the panel still has to clean up after the core.
                await Self.discard(backup: handle, library: library)
                return
            }
            try await Self.copy(from: URL(fileURLWithPath: handle.path()), to: url)
            await Self.discard(backup: handle, library: library)
            backup = nil
            savedPath = url.path
            status.show(L10n.text(.backupSaved))
        } catch {
            failure = describe(error)
            if let backup {
                await Self.discard(backup: backup, library: library)
            }
        }
    }

    func revealSaved() {
        guard let savedPath else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: savedPath)])
    }

    // MARK: - Restore

    func chooseArchive() async {
        guard phase == .idle, prepared == nil else { return }
        guard let library = session.library, let limits = session.limits else {
            failure = describe(AppFailure(.errorClosed))
            return
        }
        guard let url = Self.askArchive() else { return }
        failure = nil
        restored = nil
        savedPath = nil
        phase = .inspecting
        defer {
            phase = .idle
            cancelAction = nil
        }
        var input: ArchiveInputHandle?
        do {
            let size = try Self.byteSize(of: url)
            guard size <= limits.maxArchiveBytes else {
                throw AppFailure(.errorResourceLimit)
            }
            let task = try library.createArchiveInput()
            let staged = try await task.awaitResult()
            input = staged
            try await Self.copy(from: url, to: URL(fileURLWithPath: staged.path()))
            let inspectionTask = try library.inspectArchive(input: staged)
            cancelAction = { _ = inspectionTask.cancel() }
            let prepared = try await inspectionTask.awaitResult()
            // Inspecting consumes the input, so it is the preview that owns the
            // staged copy from here on.
            input = nil
            self.prepared = prepared
            inspection = prepared.summary()
            mode = .merge
        } catch {
            failure = describe(error)
            if let input {
                try? await library.discardArchiveInput(input: input).awaitResult()
            }
        }
    }

    /// Leaves the preview without touching the library, which is what closing
    /// the sheet does.
    func discardPreview() async {
        guard let library = session.library else { return }
        let prepared = self.prepared
        self.prepared = nil
        inspection = nil
        if let prepared {
            try? await library.discardPreparedArchive(prepared: prepared).awaitResult()
        }
    }

    func restore() async {
        guard let library = session.library, let prepared, phase == .idle else { return }
        failure = nil
        phase = .restoring
        defer {
            phase = .idle
            cancelAction = nil
        }
        do {
            let task = try library.restoreArchive(prepared: prepared, mode: mode)
            cancelAction = { _ = task.cancel() }
            let summary = try await task.awaitResult()
            // The task discards the preview and completes the restore itself,
            // so the handle is gone whether it succeeded or not.
            self.prepared = nil
            inspection = nil
            restored = summary
            status.show(L10n.text(.backupRestored))
            if mode == .replace {
                // The library now lives in a different data directory behind the
                // same path, so the core instance has to reconnect.
                await session.reopen()
            } else {
                await session.refreshStatistics()
                session.announceReload()
            }
        } catch {
            failure = describe(error)
            self.prepared = nil
            inspection = nil
        }
    }

    func cancel() {
        cancelAction?()
    }

    // MARK: - Panels and files

    private static func askDestination(name: String) -> URL? {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        panel.message = L10n.text(.backupHint)
        return panel.runModal() == .OK ? panel.url : nil
    }

    private static func askArchive() -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowsMultipleSelection = false
        panel.allowsOtherFileTypes = true
        panel.message = L10n.text(.restoreBackup)
        return panel.runModal() == .OK ? panel.url : nil
    }

    private static func byteSize(of url: URL) throws -> UInt64 {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        return (attributes[.size] as? NSNumber)?.uint64Value ?? 0
    }

    /// A backup is as large as the whole library, so the copy runs off the main
    /// actor; the UI stays responsive and the panel is already closed.
    private static func copy(from source: URL, to destination: URL) async throws {
        try await Task.detached(priority: .userInitiated) {
            let manager = FileManager.default
            if manager.fileExists(atPath: destination.path) {
                try manager.removeItem(at: destination)
            }
            try manager.copyItem(at: source, to: destination)
        }.value
    }

    private static func discard(backup: BackupFileHandle, library: LibraryHandle) async {
        try? await library.discardBackup(backup: backup).awaitResult()
    }
}
