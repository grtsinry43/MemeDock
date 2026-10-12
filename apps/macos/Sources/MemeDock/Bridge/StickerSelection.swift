import Combine
import Foundation
import MemeDockBridge

/// Multi-selection for the grid and the batch operation it feeds. Each target
/// keeps the reference it was selected with; a sticker in the trash also keeps
/// the revision a restore has to name.
@MainActor
final class StickerSelection: ObservableObject {
    @Published private(set) var targets: [String: BatchTarget] = [:]
    @Published private(set) var running = false
    @Published private(set) var failure: String?

    private let session: LibrarySession
    private let status: StatusCenter
    private var task: BatchTask?

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
    }

    var count: Int { targets.count }

    func isSelected(_ id: String) -> Bool { targets[id] != nil }

    func toggle(_ item: StickerItem) {
        if targets[item.id] != nil {
            targets[item.id] = nil
        } else {
            targets[item.id] = target(for: item)
        }
    }

    func select(_ items: [StickerItem]) {
        for item in items {
            targets[item.id] = target(for: item)
        }
    }

    func clear() {
        targets.removeAll()
    }

    func run(_ action: BatchAction) async {
        guard !running, !targets.isEmpty else { return }
        guard let library = session.library else {
            status.show(L10n.text(.errorClosed), isFailure: true)
            return
        }
        running = true
        failure = nil
        defer {
            running = false
            task = nil
        }
        do {
            let batch = try library.batch(targets: Array(targets.values), action: action)
            task = batch
            let report = try await batch.awaitResult()
            status.show(Self.summary(of: report))
            // The notification stream refreshes the grid. The selection is
            // dropped because its rows may no longer exist.
            clear()
        } catch {
            failure = describe(error)
            status.show(error)
        }
    }

    func cancel() {
        _ = try? task?.cancel()
    }

    private func target(for item: StickerItem) -> BatchTarget {
        BatchTarget(
            target: item.reference,
            deletedRevision: item.isDeleted ? item.sticker.lifecycle.revision : nil
        )
    }

    /// Reports what the core did. Rows it left `unchanged` already held the
    /// requested state, so only `failed` and `pending` deserve attention.
    private static func summary(of report: BatchReport) -> String {
        let applied = report.items.filter { $0.outcome == .applied }.count
        let failed = report.items.filter { $0.outcome == .failed }.count
        let pending = report.items.filter { $0.outcome == .pending }.count
        return L10n.text(.batchResult, applied, failed, pending)
    }
}
