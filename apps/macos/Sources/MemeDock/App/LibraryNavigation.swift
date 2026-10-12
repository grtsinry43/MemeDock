import AppKit
import Combine

/// Asks the library window to show one of its sections and come forward.
///
/// The settings window offers the same way into the trash that the library's own
/// sidebar carries, but it has no model of its own: what the two windows share is
/// the session, not the views. So the settings ask, and whichever library window
/// is open answers. The sequence makes asking twice for the section already shown
/// two different requests, so the second one is acted on instead of being dropped
/// as a repeat.
@MainActor
final class LibraryNavigation: ObservableObject {
    struct Request: Equatable {
        let section: LibraryFilter
        let sequence: Int
    }

    @Published private(set) var request: Request?

    /// Filled in by whichever library window is open.
    let library = HostWindow()

    private var sequence = 0

    func show(_ section: LibraryFilter) {
        sequence += 1
        request = Request(section: section, sequence: sequence)
    }

    /// Brings the library forward. A window behind the settings, or one that was
    /// minimised while they were open, has to come back: showing a section in a
    /// window nobody can see is not the same as taking the user to the trash.
    func bringForward() {
        NSApp.activate(ignoringOtherApps: true)
        // Ordering a miniaturised window front also brings it back from the Dock.
        library.window?.makeKeyAndOrderFront(nil)
    }
}
