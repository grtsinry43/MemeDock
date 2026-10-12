import AppKit
import SwiftUI

/// Answers the standard Edit menu commands on behalf of the library.
///
/// AppKit decides whether 拷贝 and 全选 apply by asking the responder chain, so a
/// responder that answers `copy:` and `selectAll:` is all it takes for the stock
/// items to cover the grid: they grey out when the library has nothing to act
/// on, and while a text field is being edited its field editor answers first, so
/// the keys keep the meaning they have in every other macOS text field. Nothing
/// here binds a key of its own or watches the event stream.
final class LibraryResponder: NSResponder, NSMenuItemValidation {
    private weak var model: LibraryModel?

    init(model: LibraryModel) {
        self.model = model
        super.init()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    /// ⌘A picks what is loaded, the same batch the toolbar's 选择 and the batch
    /// pane work on.
    override func selectAll(_ sender: Any?) {
        MainActor.assumeIsolated { model?.selectAll() }
    }

    /// ⌘C copies the focused sticker. `@objc` because NSResponder only declares
    /// the selectors it implements itself, and the chain can only find what the
    /// runtime can see.
    @objc func copy(_ sender: Any?) {
        MainActor.assumeIsolated {
            guard let model else { return }
            Task { await model.copyFocused() }
        }
    }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        MainActor.assumeIsolated {
            guard let model else { return false }
            if menuItem.action == #selector(selectAll(_:)) { return model.canSelectAll }
            if menuItem.action == #selector(copy(_:)) { return model.copyTarget != nil }
            return true
        }
    }
}

/// Puts the library's responder into its window's chain.
///
/// Nothing is focused in a fresh window, so the chain stops at the window itself.
/// Slotting the responder in right behind it is what lets the standard items find
/// the library, and staying behind the field editor is what leaves text editing
/// the keys it already owns.
struct LibraryResponderInstaller: NSViewRepresentable {
    let model: LibraryModel

    func makeCoordinator() -> LibraryResponder {
        LibraryResponder(model: model)
    }

    func makeNSView(context: Context) -> NSView {
        ChainHostView(responder: context.coordinator)
    }

    func updateNSView(_ view: NSView, context: Context) {}
}

/// An invisible view whose only job is to notice which window it landed in.
private final class ChainHostView: NSView {
    private let responder: LibraryResponder
    private weak var hostWindow: NSWindow?

    init(responder: LibraryResponder) {
        self.responder = responder
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window !== hostWindow else { return }
        if let hostWindow, hostWindow.nextResponder === responder {
            hostWindow.nextResponder = responder.nextResponder
        }
        responder.nextResponder = nil
        guard let window else {
            hostWindow = nil
            return
        }
        hostWindow = window
        responder.nextResponder = window.nextResponder
        window.nextResponder = responder
    }
}
