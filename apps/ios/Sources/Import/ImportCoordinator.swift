import Foundation
import MemeDockBridge
import Observation

@MainActor
@Observable
final class ImportCoordinator {
    private(set) var items: [ImportItem] = []
    private(set) var isRunning = false
    private(set) var isStopping = false
    private(set) var selectionError: String?
    private(set) var cleanupErrors: [ImportCleanupIssue] = []

    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let maximumBytes: UInt64
    @ObservationIgnored private let didFinish: @MainActor () async -> Void
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var cancellation = ImportCancellation()
    @ObservationIgnored private var transferProgress: Progress?
    @ObservationIgnored private var activeImport: ImportTask?
    @ObservationIgnored private var accepting = true

    var completed: Int { items.filter { $0.status.isFinished }.count }
    var added: Int { items.filter { $0.status == .created }.count }
    var reused: Int { items.filter { $0.status == .reused }.count }
    var problems: [ImportItem] { items.filter { $0.status.isProblem } }
    var hasProblems: Bool { !problems.isEmpty || !cleanupErrors.isEmpty }

    init(library: LibraryHandle, maximumBytes: UInt64, didFinish: @escaping @MainActor () async -> Void = {}) {
        self.library = library
        self.maximumBytes = maximumBytes
        self.didFinish = didFinish
    }

    func start(_ sources: [ImportSource]) {
        guard accepting, !isRunning else { return }
        guard !sources.isEmpty else { return }
        // Match Android's batch admission bound; no eager copies of an entire selection.
        guard sources.count <= 200 else {
            selectionError = String(localized: "Select up to 200 files at a time.")
            return
        }
        selectionError = nil
        cleanupErrors = []
        items = sources.map { ImportItem(name: $0.name) }
        cancellation = ImportCancellation()
        isStopping = false
        isRunning = true
        work = Task {
            for (index, source) in sources.enumerated() {
                if cancellation.isCancelled {
                    items[index].status = .cancelled
                    continue
                }
                await run(source, at: index)
            }
            transferProgress = nil
            activeImport = nil
            isRunning = false
            isStopping = false
            await didFinish()
        }
    }

    func selectionFailed(_ error: any Error) {
        selectionError = BridgeErrors.importDescription(error)
    }

    func dismissSelectionError() { selectionError = nil }

    func dismissReport() {
        guard !isRunning else { return }
        items = []
        cleanupErrors = []
    }

    func cancel() {
        guard isRunning else { return }
        isStopping = true
        cancellation.cancel()
        transferProgress?.cancel()
        _ = activeImport?.cancel()
        // Keep awaiting the actual result: commit-in-progress may still succeed.
    }

    func waitUntilFinished() async { await work?.value }

    func stopAndWait() async {
        accepting = false
        cancel()
        await work?.value
    }

    private func run(_ source: ImportSource, at index: Int) async {
        var input: ImportInputHandle?
        do {
            items[index].status = .reading
            let created = try await library.createImportInput().awaitResult()
            input = created
            try cancellation.check()
            let name = try await source.write(
                to: URL(fileURLWithPath: created.path()), maximumBytes: maximumBytes,
                cancellation: cancellation, transferProgress: { transferProgress = $0 }
            )
            items[index].name = name
            try cancellation.check()
            items[index].status = .importing
            // importStaged consumes the single-use token even if admission fails.
            input = nil
            let task = try library.importStaged(
                input: created, options: ImportOptions(originalName: name, title: nil, collectionId: nil)
            )
            activeImport = task
            let result = try await task.awaitResult()
            items[index].stickerID = result.sticker.id
            switch result.status {
            case .created: items[index].status = .created
            case .reused: items[index].status = .reused
            case .restoreRequired: items[index].status = .restoreRequired
            }
        } catch {
            if error is CancellationError || Self.isCoreCancellation(error) {
                items[index].status = .cancelled
            } else {
                items[index].status = .failed(BridgeErrors.importDescription(error))
            }
        }
        transferProgress = nil
        activeImport = nil
        if let input {
            do { try await library.discardImportInput(input: input).awaitResult() }
            catch { cleanupErrors.append(ImportCleanupIssue(message: BridgeErrors.importDescription(error))) }
        }
    }

    private static func isCoreCancellation(_ error: any Error) -> Bool {
        if case BridgeError.Failure(let code, _) = error { return code == .cancelled }
        let native = error as NSError
        return native.domain == NSCocoaErrorDomain && native.code == NSUserCancelledError
    }
}
