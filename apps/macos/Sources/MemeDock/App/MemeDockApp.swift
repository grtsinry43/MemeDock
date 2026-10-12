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
        WindowGroup {
            RootView(
                session: delegate.session,
                status: delegate.status,
                clipboard: delegate.clipboard,
                navigation: delegate.navigation
            )
        }
        .defaultSize(width: 1080, height: 720)
        .commands {
            CommandGroup(replacing: .newItem) {}
            LibraryCommands()
        }

        // Appearance, language, the library's own facts and the operations that
        // reach outside it are about the app, not about the sticker being
        // browsed, so they live in the settings window (Command-,) instead of
        // taking room from the grid.
        Settings {
            SettingsView(
                session: delegate.session,
                status: delegate.status,
                navigation: delegate.navigation
            )
            .font(MemeDockTypography.body)
            .buttonStyle(MemeDockButtonStyle())
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let session = LibrarySession(paths: .standard())
    let status = StatusCenter()
    /// Shared because the settings window reaches the library through it.
    let navigation = LibraryNavigation()
    private(set) lazy var clipboard = ClipboardGuard(session: session)

    private var terminating = false

    func applicationDidFinishLaunching(_ notification: Foundation.Notification) {
        // A binary started outside a bundle (swift run) still needs to become a
        // regular app before it can show a window and take focus.
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        // Applied before the first window renders so the launch never flashes
        // the wrong theme. Later changes are handled by the root view.
        NSApp.appearance = Preferences.shared.appearance.appearance
        L10n.language = Preferences.shared.language.resolved
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminating else { return .terminateNow }
        terminating = true
        clipboard.beginShutdown()
        let session = self.session
        Task {
            await session.shutdown()
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}
