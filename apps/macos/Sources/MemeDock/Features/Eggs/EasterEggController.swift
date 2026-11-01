import Combine
import Foundation

/// Owns the seven-tap brand easter egg and its transient toast state.
@MainActor
final class EasterEggController: ObservableObject {
    @Published private(set) var found = false

    private var sequence = TapSequence()
    private var dismissal: Task<Void, Never>?

    func tap() {
        guard sequence.register(at: Date()) else { return }
        found = true
        dismissal?.cancel()
        dismissal = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 4_000_000_000)
            guard !Task.isCancelled else { return }
            self?.found = false
        }
    }

    func dismiss() {
        dismissal?.cancel()
        found = false
    }
}
