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

    /// Mirrors `memedock_core::batch::MAX_BATCH_ITEMS`: one call carries this
    /// many stickers, so the pane states the limit up front instead of letting
    /// the core refuse a selection that was already too large.
    static let limit = 1000

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
            // The notification stream refreshes the grid. A sticker the core
            // settled leaves the selection because its row may no longer exist;
            // one it failed or never reached stays picked, so the call can be
            // repeated on that one alone.
            for item in report.items where Self.settled(item.outcome) {
                targets[item.id] = nil
            }
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

    /// What the core is done with: an unchanged row already held the state that
    /// was asked for, so both it and an applied one can leave the selection.
    private static func settled(_ outcome: BatchOutcome) -> Bool {
        outcome == .applied || outcome == .unchanged
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
