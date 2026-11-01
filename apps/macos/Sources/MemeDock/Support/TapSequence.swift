import Foundation

/// A rolling tap window shared by the brand easter egg. A value type so the
/// timing rules can be exercised without a view.
struct TapSequence {
    let required: Int
    let window: TimeInterval
    private var taps: [Date] = []

    init(required: Int = 7, window: TimeInterval = 3) {
        self.required = required
        self.window = window
    }

    /// Records one tap and returns true when the sequence reaches its target.
    mutating func register(at date: Date) -> Bool {
        taps.removeAll { date.timeIntervalSince($0) > window }
        taps.append(date)
        guard taps.count >= required else { return false }
        taps.removeAll()
        return true
    }
}
