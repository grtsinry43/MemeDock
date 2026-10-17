import AppKit
import Combine
import QuickLookUI
import SwiftUI

/// Answers the standard Edit menu commands on behalf of the library.
///
/// AppKit decides whether 拷贝 and 全选 apply by asking the responder chain, so a
/// responder that answers `copy:` and `selectAll:` is all it takes for the stock
/// items to cover the grid: they grey out when the library has nothing to act
/// on, and while a text field is being edited its field editor answers first, so
/// the keys keep the meaning they have in every other macOS text field. Nothing
/// here binds a key of its own or watches the event stream.
///
/// The arrow keys, Return, Space and ⌘⌫ arrive the same way.
final class LibraryResponder: NSResponder, NSMenuItemValidation, QLPreviewPanelDataSource {
    private enum KeyCode {
        static let left: UInt16 = 123
        static let right: UInt16 = 124
        static let down: UInt16 = 125
        static let up: UInt16 = 126
        static let `return`: UInt16 = 36
        static let space: UInt16 = 49
        static let delete: UInt16 = 51
    }

    private weak var model: LibraryModel?
    private var quickLookItem: StickerPreviewItem?
    private var quickLookTask: Task<Void, Never>?
    /// The copy the panel is reading; replaced or removed with the next item.
    private var quickLookDirectory: URL?
    private var subscriptions: Set<AnyCancellable> = []

    init(model: LibraryModel) {
        self.model = model
        super.init()
        // While Quick Look is open it follows the cursor without reopening.
        MainActor.assumeIsolated {
            model.$focusedId
                .dropFirst()
                .sink { [weak self] _ in
                    Task { @MainActor in self?.refreshQuickLook() }
                }
                .store(in: &subscriptions)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    // MARK: - Keys

    override func keyDown(with event: NSEvent) {
        let handled = MainActor.assumeIsolated { handle(event) }
        if !handled {
            super.keyDown(with: event)
        }
    }

    @MainActor
    private func handle(_ event: NSEvent) -> Bool {
        guard let model else { return false }
        // Arrow keys also carry .function, so only these modifiers matter.
        let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
        switch event.keyCode {
        case KeyCode.left: guard flags.isEmpty else { return false }; model.moveFocus(.left)
        case KeyCode.right: guard flags.isEmpty else { return false }; model.moveFocus(.right)
        case KeyCode.down: guard flags.isEmpty else { return false }; model.moveFocus(.down)
        case KeyCode.up: guard flags.isEmpty else { return false }; model.moveFocus(.up)
        case KeyCode.return: guard flags.isEmpty else { return false }; model.openFocused()
        case KeyCode.space: guard flags.isEmpty else { return false }; toggleQuickLook()
        case KeyCode.delete: guard flags == .command else { return false }; Task { await model.deleteFocused() }
        default: return false
        }
        return true
    }

    /// ⌘A picks what is loaded, the same batch the toolbar's 选择 and the batch
    /// pane work on.
    override func selectAll(_ sender: Any?) {
        MainActor.assumeIsolated { model?.selectAll() }
    }

    /// ⌘C copies the cursor's sticker. `@objc` so the responder chain can find
    /// the selector.
    @objc func copy(_ sender: Any?) {
        MainActor.assumeIsolated {
            guard let model else { return }
            Task { await model.copyFocused() }
        }
    }

    /// ⌘V imports the clipboard contents.
    @objc func paste(_ sender: Any?) {
        MainActor.assumeIsolated { model?.paste() }
    }

    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        MainActor.assumeIsolated {
            guard let model else { return false }
            if menuItem.action == #selector(selectAll(_:)) { return model.canSelectAll }
            if menuItem.action == #selector(copy(_:)) { return model.copyTarget != nil }
            if menuItem.action == #selector(paste(_:)) { return model.canPaste }
            return true
        }
    }

    // MARK: - Quick Look

    @MainActor
    private func toggleQuickLook() {
        guard let panel = QLPreviewPanel.shared() else { return }
        if panel.isVisible {
            panel.orderOut(nil)
            return
        }
        loadQuickLook { [weak panel] in
            panel?.makeKeyAndOrderFront(nil)
        }
    }

    /// Resolves the sticker behind the cursor and hands it to the panel. The
    /// stored original has no extension, so the panel gets a named copy: Quick
    /// Look reads the type from the file name.
    @MainActor
    private func loadQuickLook(then show: @escaping () -> Void) {
        guard let model, let id = model.focusedId else { return }
        quickLookTask?.cancel()
        quickLookTask = Task { [weak self] in
            guard let source = await model.quickLookSource(for: id) else {
                model.status.show(L10n.text(.restoreRequired), isFailure: true)
                return
            }
            guard !Task.isCancelled else { return }
            guard let staged = await Task.detached(priority: .userInitiated, operation: { Self.stage(source) }).value else {
                model.status.show(L10n.text(.quickLookUnavailable), isFailure: true)
                return
            }
            guard !Task.isCancelled, let self else { return }
            self.installQuickLookCopy(staged)
            self.quickLookItem = StickerPreviewItem(url: staged, title: source.title)
            QLPreviewPanel.shared()?.reloadData()
            show()
        }
    }

    /// Copies a sticker into a private folder under the name Quick Look needs.
    nonisolated static func stage(_ source: LibraryModel.QuickLookSource) -> URL? {
        guard let ext = StickerExport.fileExtension(forMime: source.mime) else { return nil }
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("MemeDockQuickLook-\(UUID().uuidString)", isDirectory: true)
        let name = "\(sanitize(source.title)).\(ext)"
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let destination = directory.appendingPathComponent(name)
            try FileManager.default.copyItem(at: URL(fileURLWithPath: source.path), to: destination)
            return destination
        } catch {
            try? FileManager.default.removeItem(at: directory)
            return nil
        }
    }

    private nonisolated static func sanitize(_ name: String) -> String {
        let cleaned = name
            .replacingOccurrences(of: "/", with: "-")
            .replacingOccurrences(of: ":", with: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return cleaned.isEmpty ? "Sticker" : String(cleaned.prefix(80))
    }

    /// The panel keeps showing one sticker while the cursor moves, so the
    /// previous copy is dropped only once its replacement is ready.
    @MainActor
    private func installQuickLookCopy(_ url: URL) {
        let directory = url.deletingLastPathComponent()
        if let previous = quickLookDirectory, previous != directory {
            try? FileManager.default.removeItem(at: previous)
        }
        quickLookDirectory = directory
    }

    @MainActor
    private func removeQuickLookCopy() {
        if let directory = quickLookDirectory {
            try? FileManager.default.removeItem(at: directory)
        }
        quickLookDirectory = nil
    }

    /// The panel stays open while the cursor moves.
    @MainActor
    private func refreshQuickLook() {
        guard let panel = QLPreviewPanel.shared(), panel.isVisible else { return }
        loadQuickLook {}
    }

    override func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool {
        true
    }

    override func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
        panel.dataSource = self
    }

    override func endPreviewPanelControl(_ panel: QLPreviewPanel!) {
        panel.dataSource = nil
        quickLookTask?.cancel()
        quickLookTask = nil
        quickLookItem = nil
        MainActor.assumeIsolated { removeQuickLookCopy() }
    }

    func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int {
        quickLookItem == nil ? 0 : 1
    }

    func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> (any QLPreviewItem)! {
        quickLookItem
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
