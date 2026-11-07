import AppKit
import MemeDockBridge
import SwiftUI
import UniformTypeIdentifiers

/// AppKit supplies the drag-end callback that SwiftUI's onDrag lacks. The
/// source owns its lease through the full drag, including cancelled drops.
struct QuickPickerDragSource: NSViewRepresentable {
    let item: StickerItem
    let model: QuickPickerModel
    let openLibrary: (String?) -> Void
    let dragChanged: (Bool, Bool) -> Void

    func makeNSView(context: Context) -> Source { Source() }

    func updateNSView(_ view: Source, context: Context) {
        view.item = item
        view.model = model
        view.openLibrary = openLibrary
        view.dragChanged = dragChanged
        view.setAccessibilityElement(false)
    }

    static func dismantleNSView(_ view: Source, coordinator: ()) {
        view.preparation?.cancel()
    }

    final class Source: NSView, NSDraggingSource {
        var item: StickerItem?
        weak var model: QuickPickerModel?
        var openLibrary: ((String?) -> Void)?
        var dragChanged: ((Bool, Bool) -> Void)?
        var preparation: Task<Void, Never>?
        private var lease: ArtifactLeaseHandle?
        private var down: CGPoint?
        private var latestEvent: NSEvent?
        private var attemptedDrag = false
        private var dragActive = false

        override var acceptsFirstResponder: Bool { false }
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

        override func mouseDown(with event: NSEvent) {
            guard model?.acceptsInput == true, model?.actions.busy == false, item?.asset != nil else { return }
            down = event.locationInWindow
            attemptedDrag = false
            if let item { model?.focusedId = item.id }
        }

        override func mouseUp(with event: NSEvent) {
            guard down != nil else { return }
            down = nil
            if !dragActive {
                preparation?.cancel()
                preparation = nil
                if attemptedDrag { dragChanged?(false, false) }
                else if let item { model?.activate(item) }
            }
        }

        override func mouseDragged(with event: NSEvent) {
            latestEvent = event
            guard let down, !dragActive, preparation == nil, !attemptedDrag,
                  hypot(event.locationInWindow.x - down.x, event.locationInWindow.y - down.y) > 4,
                  let item, item.asset != nil, let model, model.acceptsInput, !model.actions.busy else { return }
            attemptedDrag = true
            model.status.clear()
            dragChanged?(true, false)
            let presentation = model.presentation
            preparation = Task { [weak self] in
                do {
                    let lease = try await model.actions.prepareDragFile(item)
                    try Task.checkCancellation()
                    guard let self, model.acceptsInput, model.presentation == presentation,
                          self.down != nil, NSEvent.pressedMouseButtons & 1 != 0,
                          let event = self.latestEvent else {
                        self?.dragChanged?(false, false)
                        self?.preparation = nil
                        return
                    }
                    let artifact = try lease.metadata()
                    let file = URL(fileURLWithPath: artifact.path)
                    guard FileManager.default.fileExists(atPath: file.path) else { throw AppFailure(.errorIo) }
                    self.lease = lease
                    let writer = FileDragWriter(lease: lease, artifact: artifact)
                    let draggingItem = NSDraggingItem(pasteboardWriter: writer)
                    let image = model.thumbnails.image(for: item) ?? NSWorkspace.shared.icon(forFile: file.path)
                    draggingItem.setDraggingFrame(self.bounds.insetBy(dx: 6, dy: 6), contents: image)
                    self.dragActive = true
                    self.beginDraggingSession(with: [draggingItem], event: event, source: self)
                    self.preparation = nil
                } catch {
                    guard let self else { return }
                    self.preparation = nil
                    self.dragChanged?(false, false)
                    if !(error is CancellationError) { model.reportFailure(.dragFailed, error) }
                }
            }
        }

        func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation {
            .copy
        }

        func draggingSession(_ session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
            dragActive = false
            down = nil
            lease = nil
            if !operation.isEmpty, let item { model?.recordDrag(item) }
            dragChanged?(false, !operation.isEmpty)
        }

        override func menu(for event: NSEvent) -> NSMenu? {
            let menu = NSMenu()
            for (key, selector) in [(L10nKey.pasteImage, #selector(pasteImage)), (.copyOnly, #selector(copyImage)), (.copyFile, #selector(copyFile)), (.saveAs, #selector(save)), (.openInLibrary, #selector(openItem))] {
                let entry = NSMenuItem(title: L10n.text(key), action: selector, keyEquivalent: "")
                entry.target = self
                entry.isEnabled = model?.actions.busy == false
                menu.addItem(entry)
            }
            menu.autoenablesItems = false
            return menu
        }

        @objc private func copyImage() { if let item { model?.copy(item) } }
        @objc private func pasteImage() { if let item { model?.activate(item) } }
        @objc private func copyFile() { if let item { model?.copy(item, asFile: true) } }
        @objc private func save() { if let item { model?.save(item) } }
        @objc private func openItem() { openLibrary?(item?.id) }
    }
}

/// Offer both a file URL and the original image representation. Destinations
/// that ask for image bytes get the exported format, including GIF animation;
/// file-only destinations do not make us eagerly read the complete image.
private final class FileDragWriter: NSObject, NSPasteboardWriting {
    private let lease: ArtifactLeaseHandle
    private let url: URL
    private let imageType: NSPasteboard.PasteboardType?

    init(lease: ArtifactLeaseHandle, artifact: ExportArtifact) {
        self.lease = lease
        url = URL(fileURLWithPath: artifact.path)
        imageType = UTType(mimeType: artifact.mime).map { NSPasteboard.PasteboardType($0.identifier) }
    }

    func writableTypes(for pasteboard: NSPasteboard) -> [NSPasteboard.PasteboardType] {
        [.fileURL, .URL] + (imageType.map { [$0] } ?? [])
    }

    func writingOptions(forType type: NSPasteboard.PasteboardType, pasteboard: NSPasteboard) -> NSPasteboard.WritingOptions {
        type == imageType ? .promised : []
    }

    func pasteboardPropertyList(forType type: NSPasteboard.PasteboardType) -> Any? {
        if type == .fileURL || type == .URL { return url.absoluteString }
        guard type == imageType else { return nil }
        return try? Data(contentsOf: url, options: .mappedIfSafe)
    }
}
