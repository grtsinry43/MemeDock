import Foundation

enum Format {
    private static let bytes: ByteCountFormatter = {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        formatter.allowedUnits = [.useKB, .useMB, .useGB]
        return formatter
    }()

    private static let day: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()

    static func size(_ value: Int64) -> String {
        bytes.string(fromByteCount: value)
    }

    static func size(_ value: UInt64) -> String {
        bytes.string(fromByteCount: Int64(clamping: value))
    }

    static func timestamp(_ milliseconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(milliseconds) / 1000)
        return day.string(from: date)
    }

    /// Relative last-use text, falling back to the absolute date beyond a week.
    static func relative(_ milliseconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(milliseconds) / 1000)
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        if Date().timeIntervalSince(date) < 7 * 24 * 3600 {
            return formatter.localizedString(for: date, relativeTo: Date())
        }
        return day.string(from: date)
    }
}
