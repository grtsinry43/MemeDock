import CoreGraphics
import Foundation
import Synchronization

enum ImportItemStatus: Equatable {
    case queued, reading, importing, created, reused, restoreRequired, cancelled
    case failed(String)

    var isProblem: Bool {
        switch self {
        case .failed, .restoreRequired, .cancelled: true
        default: false
        }
    }

    var isFinished: Bool {
        switch self {
        case .queued, .reading, .importing: false
        default: true
        }
    }
}

struct ImportItem: Identifiable {
    let id = UUID()
    var name: String
    var status: ImportItemStatus = .queued
    var stickerID: String?
    var preview: CGImage?
}

struct ImportCleanupIssue: Identifiable {
    let id = UUID()
    let message: String
}

struct ImportCleanupError: LocalizedError {
    let operation: any Error
    let cleanup: any Error

    var errorDescription: String? {
        "\(operation.localizedDescription)\n\(String(localized: "Temporary files could not be removed:")) \(cleanup.localizedDescription)"
    }
}

/// Shared by the UI cancellation action and synchronous, off-main file reads.
final class ImportCancellation: Sendable {
    private struct State {
        var cancelled = false
        var cancelRead: (@Sendable () -> Void)?
    }
    private let state = Mutex(State())

    var isCancelled: Bool { state.withLock { $0.cancelled } }
    func cancel() {
        let cancelRead = state.withLock { value in
            value.cancelled = true
            return value.cancelRead
        }
        cancelRead?()
    }
    func setReadCancellation(_ cancelRead: (@Sendable () -> Void)?) {
        let alreadyCancelled = state.withLock { value in
            value.cancelRead = cancelRead
            return value.cancelled
        }
        if alreadyCancelled { cancelRead?() }
    }
    func check() throws {
        if isCancelled { throw CancellationError() }
    }
}

enum ImportInputError: LocalizedError {
    case unavailable, tooLarge, notAFile

    var errorDescription: String? {
        switch self {
        case .unavailable: String(localized: "The selected photo could not be loaded. Try selecting it again.")
        case .tooLarge: String(localized: "This file exceeds the library’s file size limit.")
        case .notAFile: String(localized: "Select an image file rather than a folder.")
        }
    }
}
