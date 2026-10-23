import Foundation
import MemeDockBridge

enum BridgeErrors {
    static func browseDescription(_ error: any Error) -> String {
        if case let BridgeError.Failure(code, _) = error {
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
        if case let BridgeError.Failure(code, _) = error {
            switch code {
            case .invalidImage: return String(localized: "This file could not be read as an image.")
            case .resourceLimit: return String(localized: "This image exceeds the library’s resource limits.")
            case .unsupportedFormat: return String(localized: "This format isn’t supported. Select a PNG, JPEG, GIF, or WebP image.")
            case .unsupportedColorProfile: return String(localized: "This image uses an unsupported color profile.")
            case .permissionDenied: return String(localized: "This file couldn’t be accessed. Select it again in Files.")
            case .storageFull: return String(localized: "There isn’t enough storage to import this image.")
            case .io: return String(localized: "This file couldn’t be read or saved. Check its availability and available storage.")
            case .closed: return String(localized: "The library is closed. Reopen it before importing.")
            case .cancelled: return String(localized: "Import cancelled.")
            case .busy: return String(localized: "The library is busy. Try importing again.")
            default: return describe(error)
            }
        }
        return error.localizedDescription
    }

    static func describe(_ error: any Error) -> String {
        if case let BridgeError.Failure(code, detail) = error {
            return "\(code): \(detail)"
        }
        return error.localizedDescription
    }
}
