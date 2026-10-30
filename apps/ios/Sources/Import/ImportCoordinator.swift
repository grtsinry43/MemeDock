import Foundation
import ImageIO
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
    @ObservationIgnored private var retrySources: [UUID: ImportSource] = [:]
    private(set) var previousProblems: [ImportItem] = []

    var completed: Int { items.filter { $0.status.isFinished }.count }
    var added: Int { items.filter { $0.status == .created }.count }
    var reused: Int { items.filter { $0.status == .reused }.count }
    var problems: [ImportItem] { previousProblems + items.filter { $0.status.isProblem } }
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
        // Keep unresolved reports when a second batch starts. Archived reports
        // retain metadata and retry sources; only the current batch owns pixels.
        previousProblems += items.filter { $0.status.isProblem }.map { item in
            var item = item
            item.preview = nil
            return item
        }
        items = sources.map { ImportItem(name: $0.name) }
        let retained = Set(previousProblems.map(\.id))
        retrySources = retrySources.filter { retained.contains($0.key) }
        retrySources.merge(
            Dictionary(uniqueKeysWithValues: zip(items.map(\.id), sources)), uniquingKeysWith: { _, new in new })
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
        retrySources = [:]
        previousProblems = []
    }

    func retryProblems() {
        let retrying = problems.filter { $0.status != .restoreRequired && retrySources[$0.id] != nil }.prefix(200)
        let sources = retrying.compactMap { retrySources[$0.id] }
        guard !isRunning, !sources.isEmpty else { return }
        let ids = Set(retrying.map(\.id))
        previousProblems.removeAll { ids.contains($0.id) }
        items.removeAll { ids.contains($0.id) }
        start(sources)
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
            items[index].preview = await ImportPreview.read(URL(fileURLWithPath: created.path()))
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
            do { try await library.discardImportInput(input: input).awaitResult() } catch {
                cleanupErrors.append(ImportCleanupIssue(message: BridgeErrors.importDescription(error)))
            }
        }
    }

    private static func isCoreCancellation(_ error: any Error) -> Bool {
        if case BridgeError.Failure(let code, _) = error { return code == .cancelled }
        let native = error as NSError
        return native.domain == NSCocoaErrorDomain && native.code == NSUserCancelledError
    }
}

private actor ImportPreview {
    static func read(_ url: URL) async -> CGImage? {
        await worker.decode(url)
    }
    private static let worker = ImportPreview()
    private func decode(_ url: URL) -> CGImage? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
            let width = properties[kCGImagePropertyPixelWidth] as? NSNumber,
            let height = properties[kCGImagePropertyPixelHeight] as? NSNumber,
            let limits = try? defaultResourceConfiguration(), width.uint64Value > 0, height.uint64Value > 0,
            width.uint64Value <= limits.maxFramePixels / height.uint64Value
        else { return nil }
        return CGImageSourceCreateThumbnailAtIndex(
            source, 0,
            [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceThumbnailMaxPixelSize: 96,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
            ] as CFDictionary)
    }
}
