import Foundation
import MemeDockBridge
import Observation

struct BackupDelivery: Identifiable {
    let id = UUID()
    let file: BackupFileHandle
}

@MainActor @Observable
final class BackupModel {
    enum Phase { case idle, creating, saving, reading, preview, restoring }
    private(set) var phase: Phase = .idle
    private(set) var summary: ArchiveSummary?
    private(set) var error: String?
    var delivery: BackupDelivery?
    var isBusy: Bool { phase != .idle && phase != .preview }
    @ObservationIgnored private var library: LibraryHandle
    @ObservationIgnored private let feedback: AppFeedback
    @ObservationIgnored private let beforeRestore: @MainActor () async throws -> Void
    @ObservationIgnored private let didRestore: @MainActor () async -> Void
    @ObservationIgnored private var prepared: PreparedArchiveHandle?
    @ObservationIgnored private var input: ArchiveInputHandle?
    @ObservationIgnored private var file: BackupFileHandle?
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var cancelCore: (@MainActor () -> Void)?
    @ObservationIgnored private var cancellation = ImportCancellation()
    @ObservationIgnored private var closed = false

    init(
        library: LibraryHandle, feedback: AppFeedback,
        beforeRestore: @escaping @MainActor () async throws -> Void, didRestore: @escaping @MainActor () async -> Void
    ) {
        self.library = library
        self.feedback = feedback
        self.beforeRestore = beforeRestore
        self.didRestore = didRestore
    }
    func replaceLibrary(_ value: LibraryHandle) { library = value }
    func selectionFailed(_ error: any Error) {
        let value = error as NSError
        if value.domain == NSCocoaErrorDomain && value.code == NSUserCancelledError { return }
        self.error = error.localizedDescription
    }
    func create() {
        guard !closed, phase == .idle else { return }
        phase = .creating
        error = nil
        cancellation = ImportCancellation()
        work = Task {
            do {
                let task = try library.createBackup()
                cancelCore = { _ = task.cancel() }
                let value = try await task.awaitResult()
                file = value
                if cancellation.isCancelled {
                    try await library.discardBackup(backup: value).awaitResult()
                    file = nil
                    phase = .idle
                } else {
                    delivery = BackupDelivery(file: value)
                    phase = .saving
                }
            } catch { failed(error) }
            cancelCore = nil
        }
    }
    func completeDelivery(saved: Bool) async {
        guard let value = file else { return }
        file = nil
        delivery = nil
        do {
            try await library.discardBackup(backup: value).awaitResult()
            if saved { feedback.post(String(localized: "Backup Saved")) }
            phase = .idle
        } catch { failed(error) }
    }
    func inspect(_ url: URL) {
        guard !closed, phase == .idle else { return }
        error = nil
        phase = .reading
        cancellation = ImportCancellation()
        work = Task {
            do {
                let value = try await library.createArchiveInput().awaitResult()
                input = value
                try await ImportFileIO.copySelectedFile(
                    from: url, to: URL(fileURLWithPath: value.path()),
                    maximumBytes: try defaultResourceConfiguration().maxArchiveBytes, cancellation: cancellation)
                try cancellation.check()
                input = nil
                let task = try library.inspectArchive(input: value)
                cancelCore = { _ = task.cancel() }
                let archive = try await task.awaitResult()
                if closed || cancellation.isCancelled {
                    try await library.discardPreparedArchive(prepared: archive).awaitResult()
                    phase = .idle
                } else {
                    prepared = archive
                    summary = archive.summary()
                    phase = .preview
                }
            } catch {
                if let value = input {
                    input = nil
                    do { try await library.discardArchiveInput(input: value).awaitResult() } catch {
                        feedback.post(BridgeErrors.managementDescription(error), kind: .error)
                    }
                }
                failed(error)
            }
            cancelCore = nil
        }
    }
    func dismissPreview() async {
        guard !closed, phase == .preview, let value = prepared else { return }
        prepared = nil
        summary = nil
        do {
            try await library.discardPreparedArchive(prepared: value).awaitResult()
            phase = .idle
        } catch { failed(error) }
    }
    func restore(_ mode: ArchiveRestoreMode) {
        guard !closed, phase == .preview, let value = prepared else { return }
        phase = .restoring
        error = nil
        cancellation = ImportCancellation()
        work = Task {
            var admitted = false
            do {
                try await beforeRestore()
                admitted = true
                try cancellation.check()
                prepared = nil
                let task = try library.restoreArchive(prepared: value, mode: mode)
                cancelCore = { _ = task.cancel() }
                _ = try await task.awaitResult()
                cancelCore = nil
                summary = nil
                if !closed {
                    await didRestore()
                    feedback.post(String(localized: "Backup Restored"))
                    phase = .idle
                }
            } catch {
                // Replacement can close the old handle before it reports failure.
                // Reconnect through the same recovery path; never reset persisted data.
                if admitted {
                    if let remaining = prepared {
                        prepared = nil
                        do { try await library.discardPreparedArchive(prepared: remaining).awaitResult() }
                        catch { feedback.post(BridgeErrors.managementDescription(error), kind: .error) }
                    }
                    summary = nil
                    if !closed { await didRestore() }
                    failed(error)
                } else {
                    phase = .preview
                    self.error = BridgeErrors.backupDescription(error)
                }
            }
            cancelCore = nil
        }
    }
    func cancel() {
        cancellation.cancel()
        cancelCore?()
    }
    func close() async {
        closed = true
        cancel()
        await work?.value
        if let value = prepared {
            prepared = nil
            do { try await library.discardPreparedArchive(prepared: value).awaitResult() } catch {
                feedback.post(BridgeErrors.managementDescription(error), kind: .error)
            }
        }
        await completeDelivery(saved: false)
    }
    private func failed(_ value: any Error) {
        phase = .idle
        if !closed, !cancellation.isCancelled {
            error = BridgeErrors.backupDescription(value)
        }
    }
}
