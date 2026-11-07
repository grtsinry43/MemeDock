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
    var onResolve: ((NSWindow) -> Void)? = nil

    func makeNSView(context: Context) -> NSView {
        Reporter(host: host, onResolve: onResolve)
    }

    func updateNSView(_ view: NSView, context: Context) {}

    private final class Reporter: NSView {
        private let host: HostWindow
        private let onResolve: ((NSWindow) -> Void)?

        init(host: HostWindow, onResolve: ((NSWindow) -> Void)?) {
            self.host = host
            self.onResolve = onResolve
            super.init(frame: .zero)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) {
            fatalError("not used")
        }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            host.window = window
            if let window {
                // Views can attach after the window's first key notification.
                // Defer published-state work until this layout update ends.
                DispatchQueue.main.async { [weak self, weak window] in
                    guard let self, let window, self.window === window else { return }
                    self.onResolve?(window)
                }
            }
        }
    }
}
