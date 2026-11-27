import AppKit
import MemeDockBridge
import UniformTypeIdentifiers

/// Exports one collection as a single archive of its originals.
///
/// The destination is chosen here; the archive is written by the core, which is
/// the only side that can read the originals and tell which of them are still
/// intact. What comes back is a report rather than a plain success: an export
/// that had to skip something still produced a usable file, so the counts and
/// the reasons are shown instead of being treated as a failure.
@MainActor
final class CollectionExport: ObservableObject {
    /// The collection being written, or nil when nothing is running.
    @Published private(set) var running: String?

    private let session: LibrarySession
    private let status: StatusCenter
    private var task: CollectionExportTask?

    var isRunning: Bool { running != nil }

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
    }

    /// Asks where the archive should go, then starts writing one collection.
    func present(_ collection: CollectionMetadata) {
        begin(
            collectionId: collection.id,
            fileName: "\(collection.name).zip",
            label: collection.name
        )
    }

    /// Every collection, one folder each inside a single archive.
    func presentAll() {
        begin(
            collectionId: nil,
            fileName: L10n.text(.exportAllFileName),
            label: L10n.text(.allCollections)
        )
    }

    private func begin(collectionId: String?, fileName: String, label: String) {
        guard !isRunning else {
            status.show(.busy)
            return
        }
        guard let library = session.library else { return }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = fileName
        panel.allowedContentTypes = [.zip]
        panel.canCreateDirectories = true
        panel.message = L10n.text(.exportCollection)
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        start(collectionId: collectionId, label: label, library: library, destination: destination)
    }

    func cancel() {
        _ = task?.cancel()
    }

    private func start(
        collectionId: String?,
        label: String,
        library: LibraryHandle,
        destination: URL
    ) {
        running = label
        status.show(L10n.text(.exportCollectionRunning, label))
        let libraryId = session.identity?.libraryId
        Task { [weak self] in
            guard let self else { return }
            defer {
                task = nil
                running = nil
            }
            do {
                let task = try library.exportCollection(
                    collectionId: collectionId,
                    destination: destination.path,
                    utcOffsetMinutes: Int32(TimeZone.current.secondsFromGMT() / 60)
                )
                self.task = task
                let summary = try await task.awaitResult()
                // A library switch closes the session this export belonged to;
                // its report no longer describes what is on screen.
                guard libraryId == session.identity?.libraryId else { return }
                report(summary)
            } catch {
                if isCancelled(error) {
                    status.clear()
                    return
                }
                // Nothing to write is a normal outcome of asking, not a failure
                // worth its error code's generic wording.
                if isEmpty(error) {
                    status.show(.collectionEmpty)
                    return
                }
                status.show(error)
            }
        }
    }

    private func report(_ summary: CollectionExportSummary) {
        guard summary.skipped > 0 else {
            status.show(L10n.text(.exportCollectionDone, Int(summary.exported)))
            return
        }
        status.show(partial(summary), isFailure: summary.exported == 0)
    }

    /// Names only the reasons that actually occurred, so the line stays
    /// readable when one sticker out of a thousand was missing.
    private func partial(_ summary: CollectionExportSummary) -> String {
        var reasons: [String] = []
        if summary.missingOriginal > 0 {
            reasons.append(L10n.text(.exportIssueMissing, Int(summary.missingOriginal)))
        }
        if summary.corrupt > 0 {
            reasons.append(L10n.text(.exportIssueCorrupt, Int(summary.corrupt)))
        }
        if summary.tooLarge > 0 {
            reasons.append(L10n.text(.exportIssueTooLarge, Int(summary.tooLarge)))
        }
        if summary.unreadable > 0 {
            reasons.append(L10n.text(.exportIssueUnreadable, Int(summary.unreadable)))
        }
        let detail = reasons.joined(separator: L10n.text(.listSeparator))
        return L10n.text(
            .exportCollectionPartial,
            Int(summary.exported),
            Int(summary.skipped),
            detail
        )
    }

    private func isEmpty(_ error: Error) -> Bool {
        guard let bridge = error as? BridgeError, case let .Failure(code, _) = bridge else {
            return false
        }
        return code == .invalidInput
    }

    private func isCancelled(_ error: Error) -> Bool {
        guard let bridge = error as? BridgeError, case let .Failure(code, _) = bridge else {
            return false
        }
        return code == .cancelled
    }
}
