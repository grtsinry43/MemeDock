import Foundation
import MemeDockBridge

enum BridgeErrors {
    static func backupDescription(_ error: any Error) -> String {
        if case let BridgeError.Failure(code, _) = error {
            switch code {
            case .corruptData, .invalidImage: return String(localized: "The backup is incomplete or damaged.")
            case .unsupportedFormat, .unsupportedSchema: return String(localized: "This backup format is not supported.")
            case .resourceLimit: return String(localized: "This backup exceeds the library’s resource limits.")
            case .conflict: return String(localized: "The library changed after the preview. Choose the backup file again.")
            case .closed, .busy: return browseDescription(error)
            default: return managementDescription(error)
            }
        }
        return error.localizedDescription
    }
    static func syncDescription(_ error: any Error) -> String {
        if case let BridgeError.Failure(code, _) = error {
            switch code {
            case .unauthorized: return String(localized: "The device token is no longer valid. Enter it again.")
            case .network, .timeout: return String(localized: "Unable to connect. Sync will retry later.")
            case .rateLimited: return String(localized: "Too many requests. Wait a moment and try again.")
            case .closed, .busy: return browseDescription(error)
            default: return String(localized: "Sync failed. Check your connection and try again.")
            }
        }
        return error.localizedDescription
    }
    static func outputDescription(_ error: any Error) -> String {
        if case BridgeError.Failure(let code, _) = error {
            switch code {
            case .permissionDenied:
                return String(localized: "Allow MemeDock to add photos in Settings, then try again.")
            case .notFound: return String(localized: "The original image is missing from this library.")
            case .storageFull: return String(localized: "There isn’t enough storage to save this image.")
            default: return String(localized: "Couldn’t prepare this image. Try another format or try again.")
            }
        }
        return error.localizedDescription
    }
    static func telegramDescription(_ error: any Error) -> String {
        if case BridgeError.Failure(let code, _) = error {
            switch code {
            case .unauthorized: return String(localized: "The token is invalid or has expired.")
            case .network, .timeout: return String(localized: "Couldn’t connect. Check your network and try again.")
            case .rateLimited: return String(localized: "Too many requests. Wait a moment and try again.")
            case .notFound: return String(localized: "This sticker pack couldn’t be found.")
            case .invalidInput: return String(localized: "Enter a valid bot token and sticker pack name or link.")
            case .cancelled: return String(localized: "Import cancelled.")
            default: return importDescription(error)
            }
        }
        return error.localizedDescription
    }
    static func managementDescription(_ error: any Error) -> String {
        if case BridgeError.Failure(let code, _) = error {
            switch code {
            case .invalidInput: return String(localized: "Enter a valid name and try again.")
            case .conflict: return String(localized: "This item changed. Refresh and try again.")
            case .entityDeleted, .notFound: return String(localized: "This item is no longer available.")
            case .storageFull: return String(localized: "There isn’t enough storage to save this change.")
            case .closed: return String(localized: "The library is closed.")
            case .cancelled: return String(localized: "The operation was cancelled.")
            default: return String(localized: "The change couldn’t be saved. Try again.")
            }
        }
        return error.localizedDescription
    }
    static func previewDescription(_ error: any Error) -> String {
        if let error = error as? PreviewError { return error.localizedDescription }
        if case BridgeError.Failure = error { return browseDescription(error) }
        return error.localizedDescription
    }

    static func originalDescription(_ code: ErrorCode?) -> String {
        switch code {
        case .notFound: String(localized: "The original image is missing from this library.")
        case .corruptData: String(localized: "The original image could not be verified.")
        case .entityDeleted: String(localized: "This sticker is no longer available.")
        default: String(localized: "The original image is unavailable for preview.")
        }
    }

    static func browseDescription(_ error: any Error) -> String {
        if case BridgeError.Failure(let code, _) = error {
            switch code {
            case .closed: return String(localized: "The library is closed. Reopen it to continue.")
            case .busy: return String(localized: "The library is busy. Try again shortly.")
            case .notFound, .entityDeleted: return String(localized: "This sticker is no longer available.")
            default: return String(localized: "The library couldn’t load this content. Try again.")
            }
        }
        return error.localizedDescription
    }

    static func importDescription(_ error: any Error) -> String {
        if case BridgeError.Failure(let code, _) = error {
            switch code {
            case .invalidImage: return String(localized: "This file could not be read as an image.")
            case .resourceLimit: return String(localized: "This image exceeds the library’s resource limits.")
            case .unsupportedFormat:
                return String(localized: "This format isn’t supported. Select a PNG, JPEG, GIF, or WebP image.")
            case .unsupportedColorProfile: return String(localized: "This image uses an unsupported color profile.")
            case .permissionDenied:
                return String(localized: "This file couldn’t be accessed. Select it again in Files.")
            case .storageFull: return String(localized: "There isn’t enough storage to import this image.")
            case .io:
                return String(
                    localized: "This file couldn’t be read or saved. Check its availability and available storage.")
            case .closed: return String(localized: "The library is closed. Reopen it before importing.")
            case .cancelled: return String(localized: "Import cancelled.")
            case .busy: return String(localized: "The library is busy. Try importing again.")
            default: return describe(error)
            }
        }
        return error.localizedDescription
    }

    static func describe(_ error: any Error) -> String {
        if case BridgeError.Failure = error {
            return managementDescription(error)
        }
        return error.localizedDescription
    }
}
