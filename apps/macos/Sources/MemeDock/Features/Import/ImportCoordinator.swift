import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Copies chosen or dropped files into core-owned staging, submits them one by
/// one, and reports what the library did with each one.
@MainActor
final class ImportCoordinator: ObservableObject {
    struct Progress: Equatable {
        let index: Int
        let total: Int
        let name: String
    }

    struct Summary {
        let created: Int
        let reused: Int
        let restoreRequired: Int
        let failed: Int
        let stopped: Bool

        var isEmpty: Bool {
            created == 0 && reused == 0 && restoreRequired == 0 && failed == 0
        }

        var message: String {
            if stopped {
                return L10n.text(.importStopped, created + reused)
            }
            if failed > 0 {
                return L10n.text(.importDonePartial, created + reused, failed)
            }
            if reused > 0 {
                return L10n.text(.importDoneReused, created, reused)
            }
            if restoreRequired > 0 {
                return L10n.text(.restoreRequired)
            }
            return L10n.text(.importDone, created)
        }
    }

    @Published private(set) var progress: Progress?
    @Published private(set) var summary: Summary?

    private let session: LibrarySession
    private let status: StatusCenter
    private var cancelRequested = false
    private var running = false
    private var currentTask: ImportTask?

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
    }

    var isRunning: Bool { running }

    func cancel() {
        cancelRequested = true
        currentTask?.cancel()
    }

    func presentPicker(collectionId: String?) {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowsOtherFileTypes = true
        panel.message = L10n.text(.importReview)
        guard panel.runModal() == .OK else { return }
        Task { [weak self] in
            await self?.importFiles(panel.urls, collectionId: collectionId)
        }
    }

    func importFiles(_ urls: [URL], collectionId: String?) async {
        guard !running else {
            status.show(.busy)
            return
        }
        let files = Self.acceptable(urls)
        guard !files.isEmpty else {
            status.show(.directorySkipped)
            return
        }
        guard let library = session.library, let limits = session.limits else {
            status.show(.errorClosed)
            return
        }
        running = true
        cancelRequested = false
        summary = nil
        defer {
            running = false
            progress = nil
            currentTask = nil
            Task { [weak self] in await self?.session.refreshStatistics() }
        }
        var created = 0
        var reused = 0
        var restoreRequired = 0
        var failed = 0
        var stopped = false

        for (index, url) in files.enumerated() {
            if cancelRequested {
                stopped = true
                break
            }
            progress = Progress(index: index + 1, total: files.count, name: url.lastPathComponent)
            do {
                let inputTask = try library.createImportInput()
                let input = try await inputTask.awaitResult()
                do {
                    try Self.stage(from: url, to: URL(fileURLWithPath: input.path()), limit: limits.maxFileBytes)
                } catch {
                    _ = try? await library.discardImportInput(input: input).awaitResult()
                    throw error
                }
                let options = ImportOptions(
                    originalName: url.lastPathComponent,
                    title: nil,
                    collectionId: collectionId
                )
                let task = try library.importStaged(input: input, options: options)
                currentTask = task
                let outcome = try await task.awaitResult()
                switch outcome.status {
                case .created: created += 1
                case .reused: reused += 1
                case .restoreRequired: restoreRequired += 1
                }
            } catch {
                if let bridge = error as? BridgeError,
                   case let .failure(code: code, detail: _) = bridge,
                   code == .cancelled {
                    stopped = true
                    break
                }
                failed += 1
            }
        }
        let summary = Summary(
            created: created,
            reused: reused,
            restoreRequired: restoreRequired,
            failed: failed,
            stopped: stopped
        )
        self.summary = summary
        if !summary.isEmpty {
            status.show(summary.message, isFailure: failed > 0)
        }
    }

    /// Dropped folders and unreadable entries are filtered out before staging.
    private static func acceptable(_ urls: [URL]) -> [URL] {
        urls.filter { url in
            guard url.isFileURL else { return false }
            let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .isReadableKey])
            return values?.isRegularFile == true && values?.isReadable == true
        }
    }

    private static func stage(from source: URL, to destination: URL, limit: UInt64) throws {
        let values = try source.resourceValues(forKeys: [.fileSizeKey])
        guard let size = values.fileSize, UInt64(size) <= limit else {
            throw AppFailure(.errorResourceLimit)
        }
        let manager = FileManager.default
        if manager.fileExists(atPath: destination.path) {
            try manager.removeItem(at: destination)
        }
        do {
            try manager.copyItem(at: source, to: destination)
        } catch {
            throw AppFailure(.errorIo)
        }
    }
}
