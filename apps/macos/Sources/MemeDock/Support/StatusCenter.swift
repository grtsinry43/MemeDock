import Combine
import Foundation

/// Single place for short-lived feedback ("Saved", "Copied", or a failure).
@MainActor
final class StatusCenter: ObservableObject {
    @Published private(set) var message: String?
    @Published private(set) var isFailure = false

    private var dismissal: Task<Void, Never>?

    func show(_ text: String, isFailure: Bool = false) {
        message = text
        self.isFailure = isFailure
        dismissal?.cancel()
        dismissal = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 4_000_000_000)
            guard !Task.isCancelled else { return }
            self?.message = nil
        }
    }

    func show(_ key: L10nKey) {
        show(L10n.text(key))
    }

    func show(_ error: Error) {
        show(describe(error), isFailure: true)
    }

    func clear() {
        dismissal?.cancel()
        message = nil
    }
}

/// Runs a throwing action and reports its failure through the status centre,
/// so call sites stay focused on the happy path.
@MainActor
func perform(_ status: StatusCenter, _ body: () async throws -> Void) async {
    do {
        try await body()
    } catch {
        status.show(error)
    }
}
