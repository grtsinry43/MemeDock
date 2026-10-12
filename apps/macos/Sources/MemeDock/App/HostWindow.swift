import AppKit
import SwiftUI

/// The window a view was put into, handed to whoever has to act on it.
///
/// A plain box rather than published state: the answer arrives while the window
/// is laying out, and publishing from there would be a view update inside a view
/// update. Nothing renders from it, so nothing needs to observe it.
@MainActor
final class HostWindow {
    weak var window: NSWindow?
}

/// Reports which window a view is hosted in.
///
/// The library and the settings are separate scenes with no view in common, so
/// neither can reach the other's objects. Learning its own window is what lets
/// each of them act on it: closing itself after handing a command over, or
/// coming to the front when it has been asked to show something.
struct HostWindowReader: NSViewRepresentable {
    let host: HostWindow

    func makeNSView(context: Context) -> NSView {
        Reporter(host: host)
    }

    func updateNSView(_ view: NSView, context: Context) {}

    private final class Reporter: NSView {
        private let host: HostWindow

        init(host: HostWindow) {
            self.host = host
            super.init(frame: .zero)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) {
            fatalError("not used")
        }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            host.window = window
        }
    }
}
