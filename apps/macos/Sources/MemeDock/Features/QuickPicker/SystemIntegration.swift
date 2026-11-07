import AppKit
import ApplicationServices
import Combine
import KeyboardShortcuts
import ServiceManagement

extension KeyboardShortcuts.Name {
    static let quickPicker = Self("quickPicker")
}

@MainActor
final class SystemIntegration: ObservableObject {
    let picker: QuickPickerController
    let menuPicker: QuickPickerModel
    weak var menuBarWindow: NSWindow?
    @Published var requestedSettingsTab: SettingsTab?
    @Published private(set) var accessibilityGranted = AXIsProcessTrusted()
    @Published private(set) var canPostEvents = CGPreflightPostEventAccess()
    @Published private(set) var loginStatus = SMAppService.mainApp.status
    @Published private(set) var shortcutDescription: String?
    private let status: StatusCenter
    private var activation: AnyCancellable?
    private let menuInput = InputPasteSession()
    private var menuPresentation: Task<Void, Never>?
    private var menuVersion = UUID()
    private var menuObservers: Set<AnyCancellable> = []
    private weak var previousKeyWindow: NSWindow?
    private var menuReturnApplication: NSRunningApplication?
    private var dismissingMenu = false

    var menuHandoffInProgress: Bool { menuInput.inProgress }

    init(session: LibrarySession, clipboard: ClipboardGuard, navigation: LibraryNavigation, status: StatusCenter) {
        self.status = status
        picker = QuickPickerController(session: session, clipboard: clipboard, navigation: navigation)
        menuPicker = QuickPickerModel(session: session, clipboard: clipboard, columns: 4, pageSize: 8, loadsCollections: false)
        picker.openAccessibilitySettings = { [weak self] in self?.openAccessibilitySettings() }
        menuPicker.copied = { [weak self] in self?.dismissMenuBar(restoreFocus: true) }
        menuPicker.paste = { [weak self] count in
            guard let self else { throw AppFailure(.errorClosed) }
            let window = self.menuBarWindow
            do { try await self.menuInput.paste(changeCount: count) { window?.orderOut(nil) } }
            catch {
                if !(error is CancellationError) { window?.makeKeyAndOrderFront(nil) }
                throw error
            }
        }
        shortcutChanged()
        KeyboardShortcuts.onKeyUp(for: .quickPicker) { [weak self] in
            guard let self else { return }
            let fromMenu = self.menuBarWindow?.isVisible == true
            let application = fromMenu ? self.menuReturnApplication : nil
            self.dismissMenuBar()
            self.picker.toggle(fromMenu: fromMenu, targetApplication: application)
        }
        activation = NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in self?.refresh() }
        NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)
            .sink { [weak self] notification in
                guard let self, let window = notification.object as? NSWindow,
                      window !== self.menuBarWindow, !(window is NSPanel) else { return }
                self.previousKeyWindow = window
            }.store(in: &menuObservers)
        NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didActivateApplicationNotification)
            .sink { [weak self] notification in
                guard let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication,
                      app.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return }
                self?.previousKeyWindow = nil
            }.store(in: &menuObservers)
    }

    func shortcutChanged() {
        shortcutDescription = KeyboardShortcuts.getShortcut(for: .quickPicker)?.description
    }

    func refresh() {
        accessibilityGranted = AXIsProcessTrusted()
        canPostEvents = CGPreflightPostEventAccess()
        loginStatus = SMAppService.mainApp.status
        shortcutChanged()
    }

    func requestAccessibility() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        accessibilityGranted = AXIsProcessTrustedWithOptions(options)
    }

    func openAccessibilitySettings() {
        // Opening the pane does not grant permission; macOS owns that choice.
        requestAccessibility()
        _ = CGRequestPostEventAccess()
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") else { return }
        if !NSWorkspace.shared.open(url) { status.show(.errorIo) }
    }

    func dismissMenuBar(restoreFocus: Bool = false) {
        guard !dismissingMenu else { return }
        dismissingMenu = true
        defer { dismissingMenu = false }
        menuVersion = UUID()
        menuPresentation?.cancel()
        menuPresentation = nil
        menuBarWindow?.orderOut(nil)
        menuPicker.dismiss()
        menuInput.reset()
        if restoreFocus, let app = menuReturnApplication, !app.isTerminated { app.activate(options: []) }
    }

    func beginMenuBarPresentation(in window: NSWindow?) {
        guard let window else { return }
        menuBarWindow = window
        guard window.isVisible else { return }
        guard !menuPicker.isPresented, menuPresentation == nil else { return }
        picker.dismiss(restoreFocus: false)
        refresh()
        // A library/settings window losing key status directly to this popover
        // means the user was in MemeDock, not in an old external chat window.
        let application = previousKeyWindow == nil ? picker.menuTargetApplication : NSRunningApplication.current
        menuReturnApplication = application
        let version = UUID()
        menuVersion = version
        menuPresentation = Task { [weak self] in
            guard let self else { return }
            defer { if self.menuVersion == version { self.menuPresentation = nil } }
            _ = await self.menuInput.capture(application)
            guard !Task.isCancelled, self.menuVersion == version, window.isVisible else { return }
            self.menuPicker.present()
        }
    }

    func showPickerFromMenu() {
        let application = menuReturnApplication
        dismissMenuBar()
        picker.toggle(fromMenu: true, targetApplication: application)
    }

    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            if enabled { try SMAppService.mainApp.register() }
            else { try SMAppService.mainApp.unregister() }
        } catch {
            status.show(error.localizedDescription, isFailure: true)
        }
        refresh()
    }

    func openLoginSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }

    func shutdown() {
        KeyboardShortcuts.disable(.quickPicker)
        picker.shutdown()
        menuPicker.shutdown()
        dismissMenuBar()
        activation = nil
        menuObservers.removeAll()
    }
}
