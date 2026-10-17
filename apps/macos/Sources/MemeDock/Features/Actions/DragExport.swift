import Foundation

/// One export, waited on by the drag start.
///
/// Drag targets read the offered file immediately, so the drag cannot start
/// until the export has produced it.
final class DragExport: @unchecked Sendable {
    private let condition = NSCondition()
    private var result: Result<URL, Error>?

    init(task: Task<URL, Error>) {
        Task {
            let outcome = await task.result
            store(outcome)
        }
    }

    /// Waits for the export to land. `nil` means it did not finish in time.
    func wait(timeout: TimeInterval) -> Result<URL, Error>? {
        condition.lock()
        let deadline = Date().addingTimeInterval(timeout)
        while result == nil {
            if !condition.wait(until: deadline) { break }
        }
        let outcome = result
        condition.unlock()
        return outcome
    }

    private func store(_ outcome: Result<URL, Error>) {
        condition.lock()
        result = outcome
        condition.broadcast()
        condition.unlock()
    }
}
