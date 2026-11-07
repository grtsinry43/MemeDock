import AppKit
import SwiftUI

@main
struct MemeDockApp: App {
    init() {
        MemeDockTypography.registerFonts()
    }

    /// The delegate owns the session because it also owns shutdown: the window
    /// can disappear before the core has closed, so quitting is deferred until
    /// the library releases its database.
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        WindowGroup(id: "library") {
            RootView(
                session: delegate.session,
                status: delegate.status,
                eggs: delegate.eggs,
                clipboard: delegate.clipboard,
                navigation: delegate.navigation,
                telegram: delegate.telegram,
                imports: delegate.imports
            )
            .background(LibraryWindowOpener(navigation: delegate.navigation))
        }
        .defaultSize(width: 1080, height: 720)
        // The library draws its own sidebar material, and a title bar on top of
        // it reads as a second, lighter strip that the traffic lights sit in.
        // Hiding it lets the material run to the window's top edge instead, so
        // the light row and the sidebar are one surface. The sidebar carries the
        // top inset that keeps its first row clear of the lights.
        .windowStyle(.hiddenTitleBar)
        .commands {
            LibraryCommands()
        }

        // Appearance, language, the library's own facts and the operations that
        // reach outside it are about the app, not about the sticker being
        // browsed, so they live in the settings window (Command-,) instead of
        // taking room from the grid. Its pages are native grouped forms, so
        // they keep the platform's own controls rather than the library's.
        Settings {
            SettingsView(
                session: delegate.session,
                status: delegate.status,
                navigation: delegate.navigation,
                telegram: delegate.telegram,
                sync: delegate.sync,
                integration: delegate.integration
            )
        }

        MenuBarExtra {
            MenuBarMenu(session: delegate.session, navigation: delegate.navigation, integration: delegate.integration,
                        imports: delegate.imports, telegram: delegate.telegram, sync: delegate.sync, status: delegate.status)
        } label: {
            Image(nsImage: MenuBarIcon.image)
                .renderingMode(.template)
                .accessibilityLabel("MemeDock")
        }
        .menuBarExtraStyle(.window)
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let session = LibrarySession(paths: .standard())
    let status = StatusCenter()
    let eggs = EasterEggController()
    /// Shared because the settings window reaches the library through it.
    let navigation = LibraryNavigation()
    private(set) lazy var clipboard = ClipboardGuard(session: session)
    /// Shared because the sheet lives on the library window while the saved
    /// token is managed in the settings; one instance keeps both in step.
    private(set) lazy var telegram = TelegramCoordinator(session: session, status: status)
    private(set) lazy var sync = SyncCoordinator(session: session)
    private(set) lazy var imports = ImportCoordinator(session: session, status: status)
    private(set) lazy var integration = SystemIntegration(session: session, clipboard: clipboard, navigation: navigation, status: status)

    private var terminating = false
    private var startup: Task<Void, Never>?
    private var launchedAtLogin = false

    func applicationWillFinishLaunching(_ notification: Foundation.Notification) {
        let event = NSAppleEventManager.shared().currentAppleEvent
        launchedAtLogin = event?.eventID == kAEOpenApplication
            && event?.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem
    }

    func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        !launchedAtLogin
    }

    func applicationDidFinishLaunching(_ notification: Foundation.Notification) {
        _ = sync
        _ = integration
        // A binary started outside a bundle (swift run) still needs to become a
        // regular app before it can show a window and take focus.
        NSApp.setActivationPolicy(Preferences.shared.showDockIcon ? .regular : .accessory)
        if !launchedAtLogin { NSApp.activate(ignoringOtherApps: true) }
        // Applied before the first window renders so the launch never flashes
        // the wrong theme. Later changes are handled by the root view.
        NSApp.appearance = Preferences.shared.appearance.appearance
        L10n.language = Preferences.shared.language.resolved
        startup = Task { await session.start() }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminating else { return .terminateLater }
        terminating = true
        integration.shutdown()
        clipboard.beginShutdown()
        let session = self.session
        Task {
            await startup?.value
            await sync.shutdown()
            await session.shutdown()
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}

/// Keep the SwiftUI scene action at the application boundary so menu-bar and
/// settings commands can recreate a library window after its last close.
private struct LibraryWindowOpener: View {
    @Environment(\.openWindow) private var openWindow
    let navigation: LibraryNavigation

    var body: some View {
        Color.clear.onAppear {
            navigation.openLibrary = { openWindow(id: "library") }
        }
    }
}
