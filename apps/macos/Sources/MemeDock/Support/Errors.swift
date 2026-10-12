import Foundation
import MemeDockBridge

/// Bridge failures always carry a stable code. The app shows the localized
/// meaning of that code, so a failed call never leaks an internal message.
extension ErrorCode {
    var message: String {
        switch self {
        case .network: return L10n.text(.errorNetwork)
        case .timeout: return L10n.text(.errorTimeout)
        case .unauthorized: return L10n.text(.errorUnauthorized)
        case .rateLimited: return L10n.text(.errorRateLimited)
        case .invalidInput: return L10n.text(.errorInvalidInput)
        case .unsupportedFormat: return L10n.text(.errorUnsupportedFormat)
        case .unsupportedColorProfile: return L10n.text(.errorUnsupportedColorProfile)
        case .invalidImage: return L10n.text(.errorInvalidImage)
        case .resourceLimit: return L10n.text(.errorResourceLimit)
        case .alreadyOpen: return L10n.text(.errorAlreadyOpen)
        case .closed: return L10n.text(.errorClosed)
        case .busy: return L10n.text(.errorBusy)
        case .cancelled: return L10n.text(.errorCancelled)
        case .notFound: return L10n.text(.errorNotFound)
        case .entityDeleted: return L10n.text(.errorEntityDeleted)
        case .conflict: return L10n.text(.errorConflict)
        case .unsupportedSchema: return L10n.text(.errorUnsupportedSchema)
        case .corruptData: return L10n.text(.errorCorruptData)
        case .permissionDenied: return L10n.text(.errorPermissionDenied)
        case .storageFull: return L10n.text(.errorStorageFull)
        case .io: return L10n.text(.errorIo)
        case .database: return L10n.text(.errorDatabase)
        case .internal: return L10n.text(.errorInternal)
        }
    }
}

/// Failures raised inside the app layer also only surface as message keys.
struct AppFailure: LocalizedError {
    let message: String

    init(_ key: L10nKey) {
        self.message = L10n.text(key)
    }

    var errorDescription: String? { message }
}

func describe(_ error: Error) -> String {
    if let bridge = error as? BridgeError {
        switch bridge {
        case let .Failure(code, _):
            return code.message
        }
    }
    if let failure = error as? AppFailure {
        return failure.message
    }
    return L10n.text(.errorUnknown)
}
