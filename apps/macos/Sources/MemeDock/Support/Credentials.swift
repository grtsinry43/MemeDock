import Foundation
import Security

/// Telegram bot token storage.
///
/// The token is a long-lived credential, so it is never written to user
/// defaults or to the library database. It lives in the login keychain as a
/// generic password, matching the Android keystore and the Linux keyring
/// stores; the service and account names stay identical across platforms.
///
/// The keychain calls block while macOS asks the user to authorize a build
/// whose signature changed, so they run on a private queue instead of the
/// concurrency pool or the main thread.
final class Credentials: Sendable {
    enum Failure: Error {
        case invalid
        case keychain(OSStatus)
    }

    /// Longest token accepted, matching the Linux store.
    private static let maximumLength = 256
    private static let service = "com.grtsinry43.memedock"
    private static let account = "telegram-bot"
    private static let label = "MemeDock Telegram Bot"
    private static let queue = DispatchQueue(label: "com.grtsinry43.memedock.credentials")

    func load() async throws -> String? {
        try await perform {
            var query = Self.baseQuery
            query[kSecReturnData as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            var result: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &result)
            switch status {
            case errSecSuccess:
                guard let data = result as? Data, let value = String(data: data, encoding: .utf8) else {
                    throw Failure.invalid
                }
                try Self.validate(value)
                return value
            case errSecItemNotFound:
                return nil
            default:
                throw Failure.keychain(status)
            }
        }
    }

    func save(_ token: String) async throws {
        try Self.validate(token)
        let data = Data(token.utf8)
        try await perform {
            let query = Self.baseQuery
            let update = [kSecValueData as String: data]
            switch SecItemUpdate(query as CFDictionary, update as CFDictionary) {
            case errSecSuccess:
                return
            case errSecItemNotFound:
                // Only a fresh item needs the label the keychain UI shows.
                var addition = query
                addition[kSecValueData as String] = data
                addition[kSecAttrLabel as String] = Self.label
                let added = SecItemAdd(addition as CFDictionary, nil)
                guard added == errSecSuccess else { throw Failure.keychain(added) }
            case let status:
                throw Failure.keychain(status)
            }
        }
    }

    func clear() async throws {
        try await perform {
            let status = SecItemDelete(Self.baseQuery as CFDictionary)
            guard status == errSecSuccess || status == errSecItemNotFound else {
                throw Failure.keychain(status)
            }
        }
    }

    private static var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    /// The stored value goes into an HTTP request verbatim, so the Linux
    /// restrictions apply unchanged: bounded length, no line breaks, no NUL.
    private static func validate(_ value: String) throws {
        guard !value.isEmpty,
              value.utf8.count <= maximumLength,
              !value.contains(where: { $0 == "\n" || $0 == "\r" || $0 == "\0" })
        else { throw Failure.invalid }
    }

    private func perform<T>(_ body: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            Self.queue.async {
                do {
                    continuation.resume(returning: try body())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }
}
