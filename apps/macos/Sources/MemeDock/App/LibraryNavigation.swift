import AppKit
import Combine

/// Asks the library window to do something and come forward.
///
/// The settings window has no model of its own: what it shares with the library
/// is the session, not the views. So it asks, and whichever library window is
/// open answers. The sequence makes asking twice for the same thing two
/// different requests, so the second one is acted on instead of being dropped
/// as a repeat.
@MainActor
final class LibraryNavigation: ObservableObject {
    enum Target: Equatable {
        case section(LibraryFilter)
        /// The Telegram sheet belongs to the library window, where what it
        /// imports appears.
        case telegramImport
    }

    struct Request: Equatable {
        let target: Target
        let sequence: Int
    }

    @Published private(set) var request: Request?

    /// Filled in by whichever library window is open.
    let library = HostWindow()

    private var sequence = 0

    func show(_ target: Target) {
        sequence += 1
        request = Request(target: target, sequence: sequence)
    }

    /// Brings the library forward. A window behind the settings, or one that was
    /// minimised while they were open, has to come back: answering in a window
    /// nobody can see is not the same as doing what was asked.
    func bringForward() {
        NSApp.activate(ignoringOtherApps: true)
        // Ordering a miniaturised window front also brings it back from the Dock.
        library.window?.makeKeyAndOrderFront(nil)
    }
}
