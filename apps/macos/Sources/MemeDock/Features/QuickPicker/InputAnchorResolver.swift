import ApplicationServices
import Foundation

/// Accessibility calls can wait for another process. Keep them off the UI
/// actor and bound each message; an unresponsive editor must not freeze search.
actor InputAnchorResolver {
    struct Snapshot: Sendable {
        let id: UUID
        let processIdentifier: pid_t
        let bounds: CGRect?
        let isSearchField: Bool
    }

    private struct Target {
        let snapshot: Snapshot
        let app: AXUIElement
        let field: AXUIElement?
        let window: AXUIElement?
    }

    private var target: Target?

    func capture(processIdentifier: pid_t) -> Snapshot? {
        target = nil
        guard processIdentifier != ProcessInfo.processInfo.processIdentifier,
              !Task.isCancelled, AXIsProcessTrusted() else { return nil }
        let app = AXUIElementCreateApplication(processIdentifier)
        AXUIElementSetMessagingTimeout(app, 0.1)
        let focused = element(app, kAXFocusedUIElementAttribute)
        if let focused { AXUIElementSetMessagingTimeout(focused, 0.1) }
        let role = focused.flatMap { attribute($0, kAXRoleAttribute) as? String }
        let subrole = focused.flatMap { attribute($0, kAXSubroleAttribute) as? String }
        guard subrole != kAXSecureTextFieldSubrole,
              focused.flatMap({ attribute($0, kAXEnabledAttribute) as? Bool }) != false else { return nil }
        let editable = role.map { [kAXTextFieldRole, kAXTextAreaRole, kAXComboBoxRole].contains($0) } ?? false
        let window = focused.flatMap { element($0, kAXWindowAttribute) }
            ?? element(app, kAXFocusedWindowAttribute)
        // Custom editors (including WeChat) may expose only AXWindow. Window
        // restoration still permits normal Cmd-V; missing field geometry must
        // not prevent pasting. Without either target, fail rather than guess.
        guard editable || window != nil else { return nil }
        if let window { AXUIElementSetMessagingTimeout(window, 0.1) }
        let field = editable ? focused : nil
        let snapshot = Snapshot(id: UUID(), processIdentifier: processIdentifier, bounds: field.flatMap(geometry),
                                isSearchField: subrole == kAXSearchFieldSubrole)
        target = Target(snapshot: snapshot, app: app, field: field, window: window)
        return snapshot
    }

    func restore(_ snapshot: Snapshot) -> Bool {
        guard !Task.isCancelled, AXIsProcessTrusted(), let target, target.snapshot.id == snapshot.id else { return false }
        if let window = target.window { _ = AXUIElementPerformAction(window, kAXRaiseAction as CFString) }
        if let field = target.field {
            _ = AXUIElementSetAttributeValue(field, kAXFocusedAttribute as CFString, kCFBooleanTrue)
        }
        return isFocused(snapshot)
    }

    func isFocused(_ snapshot: Snapshot) -> Bool {
        guard !Task.isCancelled, let target, target.snapshot.id == snapshot.id else { return false }
        if let field = target.field {
            guard let current = element(target.app, kAXFocusedUIElementAttribute) else { return false }
            return CFEqual(current, field)
        }
        guard let window = target.window,
              let currentWindow = element(target.app, kAXFocusedWindowAttribute),
              CFEqual(currentWindow, window) else { return false }
        if let focused = element(target.app, kAXFocusedUIElementAttribute) {
            let subrole = attribute(focused, kAXSubroleAttribute) as? String
            guard subrole != kAXSecureTextFieldSubrole, subrole != kAXSearchFieldSubrole,
                  attribute(focused, kAXEnabledAttribute) as? Bool != false else { return false }
        }
        return true
    }

    private func element(_ parent: AXUIElement, _ name: String) -> AXUIElement? {
        guard let value = attribute(parent, name), CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return (value as! AXUIElement)
    }

    private func geometry(_ focused: AXUIElement) -> CGRect? {
        guard let position = attribute(focused, kAXPositionAttribute),
              let size = attribute(focused, kAXSizeAttribute),
              CFGetTypeID(position) == AXValueGetTypeID(),
              CFGetTypeID(size) == AXValueGetTypeID() else { return nil }
        var point = CGPoint.zero
        var dimensions = CGSize.zero
        guard AXValueGetValue(position as! AXValue, .cgPoint, &point),
              AXValueGetValue(size as! AXValue, .cgSize, &dimensions),
              point.x.isFinite, point.y.isFinite,
              dimensions.width.isFinite, dimensions.height.isFinite,
              dimensions.width > 0, dimensions.height > 0 else { return nil }
        // Never read or retain the contents of the editor.
        return CGRect(origin: point, size: dimensions)
    }

    private func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
        guard !Task.isCancelled else { return nil }
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
        return value
    }
}
