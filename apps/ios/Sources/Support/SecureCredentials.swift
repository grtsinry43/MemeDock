import Foundation
import Security

actor SecureCredentials {
    static let shared = SecureCredentials()
    private let service = "com.grtsinry43.memedock.credentials"
    private func query(_ account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }
    func read(_ account: String) throws -> String? {
        var attributes = query(account)
        attributes[kSecReturnData as String] = true
        attributes[kSecMatchLimit as String] = kSecMatchLimitOne
        var value: CFTypeRef?
        let status = SecItemCopyMatching(attributes as CFDictionary, &value)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = value as? Data, let text = String(data: data, encoding: .utf8) else {
            throw CredentialError.unavailable
        }
        return text
    }
    func save(_ value: String, account: String) throws {
        let data = Data(value.utf8)
        let attributes = query(account)
        let update = [kSecValueData as String: data]
        let status = SecItemUpdate(attributes as CFDictionary, update as CFDictionary)
        if status == errSecSuccess { return }
        guard status == errSecItemNotFound else { throw CredentialError.unavailable }
        var item = attributes
        item[kSecValueData as String] = data
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        guard SecItemAdd(item as CFDictionary, nil) == errSecSuccess else { throw CredentialError.unavailable }
    }
    func remove(_ account: String) throws {
        let status = SecItemDelete(query(account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw CredentialError.unavailable }
    }
}

enum CredentialError: LocalizedError {
    case unavailable
    var errorDescription: String? { String(localized: "Unable to read or save credentials. Enter the token again.") }
}
