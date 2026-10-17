import Foundation

/// One preparation worker, one latest request, and one retained ready result.
/// Cancellation never admits a second worker until the first has returned.
@MainActor
final class DragExport<Value> {
    private struct Request {
        let key: String
        let operation: () async throws -> Value
    }
    private var worker: Task<Void, Never>?
    private var currentKey: String?
    private var queued: Request?
    private var ready: (key: String, value: Value)?
    private var failed: (key: String, error: Error)?
    var completed: ((String, Result<Value, Error>) -> Void)?

    func value(for key: String) -> Value? { ready?.key == key ? ready?.value : nil }
    func failure(for key: String) -> Error? { failed?.key == key ? failed?.error : nil }

    func prepare(key: String, operation: @escaping () async throws -> Value) {
        if ready?.key == key || currentKey == key && worker?.isCancelled == false { return }
        let request = Request(key: key, operation: operation)
        if let worker {
            queued = request
            worker.cancel()
        } else { start(request) }
    }

    func cancel(key: String) {
        if queued?.key == key { queued = nil }
        if currentKey == key { worker?.cancel() }
    }

    func invalidate() {
        ready = nil
        failed = nil
        queued = nil
        worker?.cancel()
    }

    private func start(_ request: Request) {
        currentKey = request.key
        ready = nil
        failed = nil
        worker = Task { [weak self] in
            let result: Result<Value, Error>
            do {
                try await Task.sleep(nanoseconds: 150_000_000)
                let value = try await request.operation()
                try Task.checkCancellation()
                result = .success(value)
            } catch { result = .failure(error) }
            guard let self else { return }
            self.worker = nil
            self.currentKey = nil
            if !Task.isCancelled {
                switch result {
                case let .success(value): self.ready = (request.key, value)
                case let .failure(error): self.failed = (request.key, error)
                }
                self.completed?(request.key, result)
            }
            if let queued = self.queued {
                self.queued = nil
                self.start(queued)
            }
        }
    }
}
