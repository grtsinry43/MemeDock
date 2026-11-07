import AppKit
import ApplicationServices
import Carbon.HIToolbox

/// Captures one input control per presentation. AX references stay in the
/// resolver actor; activation, clipboard checks and event posting stay on UI.
@MainActor
final class InputPasteSession {
    private(set) var inProgress = false
    private let resolver = InputAnchorResolver()
    private var snapshot: InputAnchorResolver.Snapshot?
    private var application: NSRunningApplication?
    private var version = UUID()

    func capture(_ application: NSRunningApplication?) async -> CGRect? {
        let version = UUID()
        self.version = version
        self.application = application
        snapshot = nil
        guard let application, !application.isTerminated else { return nil }
        // AX calls to our own PID can execute AppKit accessibility callbacks
        // on the caller's background thread. Our inputs are plain-text fields;
        // use local, main-actor geometry and never synthesize a paste to self.
        if application.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            return Self.localInputBounds()
        }
        let result = await resolver.capture(processIdentifier: application.processIdentifier)
        guard !Task.isCancelled, self.version == version else { return nil }
        snapshot = result
        return result?.bounds
    }

    func reset() {
        version = UUID()
        snapshot = nil
        application = nil
    }

    func paste(changeCount: Int, hide: () -> Void) async throws {
        guard !inProgress else { throw AppFailure(.errorBusy) }
        guard application?.processIdentifier != ProcessInfo.processInfo.processIdentifier else {
            throw AppFailure(.pasteUnsupportedInput)
        }
        guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else { throw AppFailure(.pastePermissionRequired) }
        guard let snapshot, let application, !application.isTerminated else { throw AppFailure(.pasteTargetMissing) }
        guard !snapshot.isSearchField else { throw AppFailure(.pasteUnsupportedInput) }
        let version = self.version
        try check(version: version, changeCount: changeCount)
        let frontmost = NSWorkspace.shared.frontmostApplication?.processIdentifier
        guard frontmost == snapshot.processIdentifier || frontmost == ProcessInfo.processInfo.processIdentifier else {
            throw AppFailure(.pasteTargetChanged)
        }
        inProgress = true
        defer { inProgress = false }
        hide()
        guard application.activate(options: []) else { throw AppFailure(.pasteTargetMissing) }

        // Wait for observed readiness, rather than sending Cmd-V after an
        // arbitrary delay. Never retry a paste once its events have been sent.
        let clock = ContinuousClock()
        let deadline = clock.now.advanced(by: .milliseconds(700))
        while NSWorkspace.shared.frontmostApplication?.processIdentifier != snapshot.processIdentifier {
            try check(version: version, changeCount: changeCount)
            guard clock.now < deadline else { throw AppFailure(.pasteTargetMissing) }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        _ = await resolver.restore(snapshot)
        while !(await resolver.isFocused(snapshot)) || !Self.modifiersReleased {
            try check(version: version, changeCount: changeCount)
            guard clock.now < deadline else { throw AppFailure(.pasteTargetMissing) }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        try check(version: version, changeCount: changeCount)
        guard !application.isTerminated,
              NSWorkspace.shared.frontmostApplication?.processIdentifier == snapshot.processIdentifier,
              CGPreflightPostEventAccess() else { throw AppFailure(.pasteTargetChanged) }
        try Self.postPaste(to: snapshot.processIdentifier)
    }

    private func check(version: UUID, changeCount: Int) throws {
        try Task.checkCancellation()
        guard self.version == version else { throw CancellationError() }
        guard NSPasteboard.general.changeCount == changeCount else { throw AppFailure(.pasteClipboardChanged) }
    }

    private static var modifiersReleased: Bool {
        CGEventSource.flagsState(.hidSystemState)
            .intersection([.maskCommand, .maskControl, .maskAlternate, .maskShift]).isEmpty
    }

    private static func localInputBounds() -> CGRect? {
        guard let window = NSApp.keyWindow, let editor = window.firstResponder as? NSTextView else { return nil }
        let view = (editor.delegate as? NSTextField) ?? editor
        guard view.window === window else { return nil }
        let frame = window.convertToScreen(view.convert(view.bounds, to: nil))
        // Match the AX coordinate convention expected by panel placement.
        let primaryTop = NSScreen.screens.first?.frame.maxY ?? 0
        return CGRect(x: frame.minX, y: primaryTop - frame.maxY, width: frame.width, height: frame.height)
    }

    private static func postPaste(to processIdentifier: pid_t) throws {
        guard processIdentifier != ProcessInfo.processInfo.processIdentifier else { throw AppFailure(.pasteUnsupportedInput) }
        guard let source = CGEventSource(stateID: .privateState),
              let commandDown = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_Command), keyDown: true),
              let keyDown = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: true),
              let keyUp = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: false),
              let commandUp = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_Command), keyDown: false) else {
            throw AppFailure(.errorInternal)
        }
        commandDown.flags = .maskCommand
        keyDown.flags = .maskCommand
        keyUp.flags = .maskCommand
        commandUp.flags = []
        for event in [commandDown, keyDown, keyUp, commandUp] { event.postToPid(processIdentifier) }
    }
}
