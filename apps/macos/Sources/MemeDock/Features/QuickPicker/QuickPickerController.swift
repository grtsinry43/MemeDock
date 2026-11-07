import AppKit
import Combine
import SwiftUI

@MainActor
final class QuickPickerController: NSObject, NSWindowDelegate {
    let model: QuickPickerModel
    var openAccessibilitySettings: (() -> Void)?
    private let navigation: LibraryNavigation
    private let input = InputPasteSession()
    private var panel: PickerPanel?
    private var presentation: Task<Void, Never>?
    private var request = UUID()
    private var previousApplication: NSRunningApplication?
    private var lastExternalApplication: NSRunningApplication?
    private var activation: AnyCancellable?
    private var menuObservations: Set<AnyCancellable> = []
    private var trackingMenus = 0
    private var outsideClick: Any?
    private var insideClick: Any?
    private var dragging = false
    private var closing = false
    private var dismissing = false

    init(session: LibrarySession, clipboard: ClipboardGuard, navigation: LibraryNavigation) {
        model = QuickPickerModel(session: session, clipboard: clipboard)
        self.navigation = navigation
        super.init()
        remember(NSWorkspace.shared.frontmostApplication)
        model.copied = { [weak self] in self?.dismiss(restoreFocus: true) }
        model.paste = { [weak self] changeCount in
            guard let self else { throw AppFailure(.errorClosed) }
            do { try await self.input.paste(changeCount: changeCount) { self.panel?.orderOut(nil) } }
            catch {
                if !(error is CancellationError) { self.panel?.makeKeyAndOrderFront(nil) }
                throw error
            }
        }
        activation = NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didActivateApplicationNotification)
            .sink { [weak self] notification in
                guard let self,
                      let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else { return }
                self.remember(app)
                if self.panel?.isVisible == true, app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
                   !self.dragging, !self.input.inProgress {
                    self.dismiss(restoreFocus: false)
                }
            }
        NotificationCenter.default.publisher(for: NSMenu.didBeginTrackingNotification)
            .sink { [weak self] _ in self?.trackingMenus += 1 }
            .store(in: &menuObservations)
        NotificationCenter.default.publisher(for: NSMenu.didEndTrackingNotification)
            .sink { [weak self] _ in
                guard let self else { return }
                self.trackingMenus = max(0, self.trackingMenus - 1)
            }
            .store(in: &menuObservations)
    }

    func toggle(fromMenu: Bool = false, targetApplication: NSRunningApplication? = nil) {
        guard !closing else { return }
        if presentation != nil || panel?.isVisible == true {
            if !dragging { dismiss(restoreFocus: true) }
        } else { show(fromMenu: fromMenu, targetApplication: targetApplication) }
    }

    var menuTargetApplication: NSRunningApplication? {
        let frontmost = NSWorkspace.shared.frontmostApplication
        return frontmost?.processIdentifier == ProcessInfo.processInfo.processIdentifier ? lastExternalApplication : frontmost
    }

    private func show(fromMenu: Bool, targetApplication: NSRunningApplication?) {
        let frontmost = NSWorkspace.shared.frontmostApplication
        remember(frontmost)
        // Opening a menu can activate our process. A hotkey pressed while
        // already editing in MemeDock, however, must keep that context.
        previousApplication = targetApplication ?? (fromMenu ? lastExternalApplication : frontmost)
        let application = previousApplication
        let mouse = NSEvent.mouseLocation
        let shouldAnchor = Preferences.shared.anchorToInput
        let request = UUID()
        self.request = request
        presentation = Task { [weak self] in
            guard let self else { return }
            let capturedBounds = await self.input.capture(application)
            let bounds = shouldAnchor ? capturedBounds : nil
            guard !Task.isCancelled, self.request == request else { return }
            self.presentation = nil
            let panel = self.makePanel()
            panel.title = L10n.text(.quickPicker)
            panel.setFrame(Self.frame(input: bounds, mouse: mouse), display: false)
            self.model.present()
            self.installClickMonitors()
            panel.makeKeyAndOrderFront(nil)
        }
    }

    func dismiss(restoreFocus: Bool) {
        guard !dismissing else { return }
        dismissing = true
        defer { dismissing = false }
        request = UUID()
        presentation?.cancel()
        presentation = nil
        removeClickMonitors()
        let wasVisible = panel?.isVisible == true
        panel?.orderOut(nil)
        model.dismiss()
        input.reset()
        if restoreFocus, wasVisible,
           let previousApplication, !previousApplication.isTerminated {
            previousApplication.activate(options: [])
        }
    }

    func shutdown() {
        closing = true
        model.shutdown()
        dismiss(restoreFocus: false)
        activation = nil
        menuObservations.removeAll()
    }

    func windowDidResignKey(_ notification: Notification) {
        // Sheets and drag sessions temporarily own keyboard focus. Their
        // completion decides what to dismiss; hiding here would break handoff.
        guard !dismissing, !dragging, !input.inProgress, trackingMenus == 0, !model.actions.busy, NSApp.modalWindow == nil else { return }
        dismiss(restoreFocus: false)
    }

    private func remember(_ app: NSRunningApplication?) {
        if let app, app.processIdentifier != ProcessInfo.processInfo.processIdentifier {
            lastExternalApplication = app
        }
    }

    private func makePanel() -> PickerPanel {
        if let panel { return panel }
        let panel = PickerPanel(
            contentRect: NSRect(x: 0, y: 0, width: 480, height: 360),
            styleMask: [.titled, .fullSizeContentView, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.delegate = self
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            panel.standardWindowButton(button)?.isHidden = true
        }
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        panel.keyHandler = { [weak self] event in
            guard let self else { return false }
            let editor = self.panel?.firstResponder as? NSTextView
            // Return/Escape belong to the input method while composing text.
            guard editor?.hasMarkedText() != true else { return false }
            let modifiers = event.modifierFlags.intersection([.command, .option, .control, .shift])
            if modifiers == .command, event.charactersIgnoringModifiers == "c", editor?.selectedRange().length ?? 0 == 0 {
                if let item = self.model.focusedItem { self.model.copy(item) }
                return true
            }
            guard modifiers.isEmpty else { return false }
            switch event.keyCode {
            case 53: self.dismiss(restoreFocus: true)
            case 36, 76:
                if let item = self.model.focusedItem { self.model.activate(item) }
            case 125: self.model.move(.down)
            case 126: self.model.move(.up)
            case 123, 124:
                // Preserve text editing; horizontal grid movement is available
                // when the query is empty or the field is no longer editing.
                guard editor == nil || editor?.string.isEmpty == true else { return false }
                self.model.move(event.keyCode == 123 ? .left : .right)
            default: return false
            }
            return true
        }
        let hosting = NSHostingView(rootView: QuickPickerView(
            model: model,
            dismiss: { [weak self] in self?.dismiss(restoreFocus: true) },
            openLibrary: { [weak self] id in
                guard let self else { return }
                self.dismiss(restoreFocus: false)
                if let id { self.navigation.show(.sticker(id)) }
                self.navigation.bringForward()
            },
            dragChanged: { [weak self] active, completed in
                guard let self else { return }
                self.dragging = active
                if !active, completed { self.dismiss(restoreFocus: false) }
                else if !active, self.panel?.isVisible == true { self.panel?.makeKeyAndOrderFront(nil) }
            },
            requestAccess: { [weak self] in self?.openAccessibilitySettings?() }
        ))
        // The controller owns the anchored panel's frame. Feedback must not
        // propagate SwiftUI minimum-size changes back into the window.
        hosting.sizingOptions = []
        panel.contentView = hosting
        self.panel = panel
        return panel
    }

    private func installClickMonitors() {
        removeClickMonitors()
        outsideClick = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.dragging, !self.input.inProgress, self.trackingMenus == 0, NSApp.modalWindow == nil else { return }
                self.dismiss(restoreFocus: false)
            }
        }
        insideClick = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            MainActor.assumeIsolated {
                if let self, event.window !== self.panel, !self.dragging, !self.input.inProgress,
                   self.trackingMenus == 0, NSApp.modalWindow == nil {
                    self.dismiss(restoreFocus: false)
                }
            }
            return event
        }
    }

    private func removeClickMonitors() {
        if let outsideClick { NSEvent.removeMonitor(outsideClick) }
        if let insideClick { NSEvent.removeMonitor(insideClick) }
        outsideClick = nil
        insideClick = nil
    }

    private static func frame(input: CGRect?, mouse: CGPoint) -> CGRect {
        // Accessibility uses a top-left origin on the primary display;
        // AppKit uses a bottom-left origin. Preserve negative secondary origins.
        let primaryTop = NSScreen.screens.first?.frame.maxY ?? 0
        let anchor = input.map {
            CGRect(x: $0.minX, y: primaryTop - $0.maxY, width: $0.width, height: $0.height)
        }
        let point = anchor.map { CGPoint(x: $0.midX, y: $0.midY) } ?? mouse
        let screen = NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) }
            ?? NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) }
            ?? NSScreen.main
        let area = (screen?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)).insetBy(dx: 12, dy: 12)
        let size = CGSize(width: min(480, area.width), height: min(360, area.height))
        var origin: CGPoint
        if let anchor, anchor.intersects(area) {
            origin = CGPoint(x: anchor.minX, y: anchor.minY - size.height - 8)
            if origin.y < area.minY { origin.y = anchor.maxY + 8 }
        } else {
            origin = CGPoint(x: mouse.x - size.width / 2, y: mouse.y - size.height - 12)
            if origin.y < area.minY { origin.y = mouse.y + 12 }
        }
        origin.x = min(max(origin.x, area.minX), area.maxX - size.width)
        origin.y = min(max(origin.y, area.minY), area.maxY - size.height)
        return CGRect(origin: origin, size: size)
    }
}

private final class PickerPanel: NSPanel {
    var keyHandler: ((NSEvent) -> Bool)?
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func sendEvent(_ event: NSEvent) {
        if event.type == .keyDown, keyHandler?(event) == true { return }
        super.sendEvent(event)
    }
}
