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
        let field: AXUIElement
        let window: AXUIElement?
    }

    private var target: Target?

    func capture(processIdentifier: pid_t) -> Snapshot? {
        target = nil
        guard processIdentifier != ProcessInfo.processInfo.processIdentifier,
              !Task.isCancelled, AXIsProcessTrusted() else { return nil }
        let app = AXUIElementCreateApplication(processIdentifier)
        AXUIElementSetMessagingTimeout(app, 0.1)
        guard let value = attribute(app, kAXFocusedUIElementAttribute),
              CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        let focused = value as! AXUIElement
        AXUIElementSetMessagingTimeout(focused, 0.1)
        guard let role = attribute(focused, kAXRoleAttribute) as? String,
              [kAXTextFieldRole, kAXTextAreaRole, kAXComboBoxRole].contains(role),
              attribute(focused, kAXSubroleAttribute) as? String != kAXSecureTextFieldSubrole,
              attribute(focused, kAXEnabledAttribute) as? Bool != false else { return nil }
        let windowValue = attribute(focused, kAXWindowAttribute)
        let window = windowValue.flatMap { CFGetTypeID($0) == AXUIElementGetTypeID() ? ($0 as! AXUIElement) : nil }
        if let window { AXUIElementSetMessagingTimeout(window, 0.1) }
        let snapshot = Snapshot(id: UUID(), processIdentifier: processIdentifier, bounds: geometry(focused),
                                isSearchField: attribute(focused, kAXSubroleAttribute) as? String == kAXSearchFieldSubrole)
        target = Target(snapshot: snapshot, app: app, field: focused, window: window)
        return snapshot
    }

    func restore(_ snapshot: Snapshot) -> Bool {
        guard !Task.isCancelled, AXIsProcessTrusted(), let target, target.snapshot.id == snapshot.id else { return false }
        if let window = target.window { _ = AXUIElementPerformAction(window, kAXRaiseAction as CFString) }
        _ = AXUIElementSetAttributeValue(target.field, kAXFocusedAttribute as CFString, kCFBooleanTrue)
        return isFocused(snapshot)
    }

    func isFocused(_ snapshot: Snapshot) -> Bool {
        guard !Task.isCancelled, let target, target.snapshot.id == snapshot.id,
              let current = attribute(target.app, kAXFocusedUIElementAttribute) else { return false }
        return CFEqual(current, target.field)
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
