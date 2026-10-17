import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Owns every hand-off of a sticker to the outside world: clipboard, drag,
/// save panel, and the share sheet. Exactly one hand-off runs at a time, so a
/// second request is rejected instead of racing the first.
@MainActor
final class StickerActions: ObservableObject {
    @Published private(set) var busy = false
    @Published private(set) var preparingOutput = false
    private var activeExport: ExportTask?

    func cancelOutput() { _ = activeExport?.cancel() }
    /// A preset picked in the detail pane for this run of the app. It replaces
    /// the copy and share defaults while set; `nil` follows the settings.
    @Published var override: ExportPreset?

    /// What copying an image or a file hands over right now.
    func copyPreset(for item: StickerItem) -> ExportPreset {
        StickerExport.effectiveOverride(override, animated: item.animated) ?? (item.animated ? preferences.copyAnimatedPreset : preferences.copyPreset)
    }
    /// What sharing and saving hand over right now.
    func sharePreset(for item: StickerItem) -> ExportPreset {
        StickerExport.effectiveOverride(override, animated: item.animated) ?? (item.animated ? preferences.shareAnimatedPreset : preferences.sharePreset)
    }
    /// What a drag hands over; the detail pane's temporary choice never
    /// changes it.
    private func dragPreset(for item: StickerItem) -> ExportPreset {
        item.animated ? preferences.dragAnimatedPreset : preferences.dragPreset
    }

    private let drag = DragExport<ArtifactLeaseHandle>()
    private var dragPromptKey: String?
    private var observations: Set<AnyCancellable> = []

    private let session: LibrarySession
    private let clipboard: ClipboardGuard
    private let status: StatusCenter
    private let preferences = Preferences.shared
    /// Leases stay alive while the pasteboard, a drag, or a share sheet points
    /// at their file.
    private var retained: [String: ArtifactLeaseHandle] = [:]

    init(session: LibrarySession, clipboard: ClipboardGuard, status: StatusCenter) {
        self.session = session
        self.clipboard = clipboard
        self.status = status
        preferences.objectWillChange.sink { [weak self] in
            self?.drag.invalidate()
            self?.objectWillChange.send()
        }.store(in: &observations)
        session.$phase.sink { [weak self] phase in
            if phase != .ready { self?.drag.invalidate() }
        }.store(in: &observations)
        session.events.sink { [weak self] event in
            switch event {
            case .reload, .sticker: self?.drag.invalidate()
            default: break
            }
        }.store(in: &observations)
        drag.completed = { [weak self] key, result in
            guard let self, self.dragPromptKey == key else { return }
            self.dragPromptKey = nil
            switch result {
            case .success: self.status.show(.dragReady)
            case let .failure(error): self.status.show(error)
            }
        }
    }

    func copyImage(_ item: StickerItem, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.copyPreset(for: item), firstFrame: firstFrame)
            let artifact = try lease.metadata()
            try await self.clipboard.install(lease: lease) { pasteboard in
                try Self.write(artifact: artifact, representation: .image, to: pasteboard)
            }
            try await self.record(item.id, .copyImage)
            self.status.show(.imageCopied)
        }
    }

    func copyFile(_ item: StickerItem) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.copyPreset(for: item), firstFrame: false)
            let artifact = try lease.metadata()
            try await self.clipboard.install(lease: lease) { pasteboard in
                try Self.write(artifact: artifact, representation: .file, to: pasteboard)
            }
            try await self.record(item.id, .copyFile)
            self.status.show(.fileCopied)
        }
    }

    func save(_ item: StickerItem, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.sharePreset(for: item), firstFrame: firstFrame)
            let artifact = try lease.metadata()
            guard let destination = try await Self.askDestination(name: artifact.fileName) else { return }
            try Self.replace(source: URL(fileURLWithPath: artifact.path), destination: destination)
            try await self.record(item.id, .exportSaved)
            self.status.show(.saved)
        }
    }

    func share(_ item: StickerItem, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.sharePreset(for: item), firstFrame: firstFrame)
            let artifact = try lease.metadata()
            let url = URL(fileURLWithPath: artifact.path)
            guard let view = Self.anchorView() else { throw AppFailure(.errorInternal) }
            let picker = NSSharingServicePicker(items: [url])
            picker.show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
            try await self.record(item.id, .shareLaunched)
            self.status.show(.shareLaunched)
        }
    }

    /// Prepares the file a drag hands over.
    ///
    /// The export runs to completion first: some destinations read the offered
    /// data immediately and ignore a promise. `NSItemProvider(contentsOf:)`
    /// then derives the file's types and serves its bytes.
    func dragProvider(for item: StickerItem) -> NSItemProvider {
        guard session.library != nil, !item.isDeleted else { return NSItemProvider() }
        let key = dragPreparationKey(for: item)
        if let lease = drag.value(for: key), let artifact = try? lease.metadata(), FileManager.default.fileExists(atPath: artifact.path) {
            _ = retain(lease)
            return NSItemProvider(contentsOf: URL(fileURLWithPath: artifact.path)) ?? NSItemProvider()
        }
        if drag.value(for: key) != nil { drag.invalidate() }
        if let error = drag.failure(for: key) {
            status.show(error)
        } else {
            dragPromptKey = key
            status.show(.dragPreparing)
        }
        prepareDrag(for: item)
        return NSItemProvider()
    }

    func dragPreparationKey(for item: StickerItem) -> String {
        "\(session.library.map { String(describing: ObjectIdentifier($0)) } ?? "closed"):\(item.id):\(item.sticker.lifecycle.generation):\(item.animated):\(StickerExport.storageKey(dragPreset(for: item)))"
    }

    func prepareDrag(for item: StickerItem) {
        guard let library = session.library, !item.isDeleted, item.asset != nil else { return }
        let key = dragPreparationKey(for: item)
        let resolution = StickerExport.resolve(preset: dragPreset(for: item), animated: item.animated, firstFrame: false)
        let options = StickerExport.options(resolution.preset, animation: resolution.animation)
        drag.prepare(key: key) {
            let task = try library.export(id: item.id, options: options)
            return try await withTaskCancellationHandler {
                let lease = try await task.awaitResult()
                try Task.checkCancellation()
                return lease
            } onCancel: { _ = task.cancel() }
        }
    }

    func cancelDragPreparation(for item: StickerItem) {
        let key = dragPreparationKey(for: item)
        if dragPromptKey != key { drag.cancel(key: key) }
    }

    private enum Representation {
        case image
        case file
    }

    private func export(
        _ item: StickerItem,
        preset: ExportPreset,
        firstFrame: Bool
    ) async throws -> ArtifactLeaseHandle {
        let resolution = StickerExport.resolve(preset: preset, animated: item.animated, firstFrame: firstFrame)
        let options = StickerExport.options(resolution.preset, animation: resolution.animation)
        let task = try session.requireLibrary().export(id: item.id, options: options)
        activeExport = task
        preparingOutput = true
        defer { activeExport = nil; preparingOutput = false }
        let lease = try await withTaskCancellationHandler {
            try await task.awaitResult()
        } onCancel: { _ = task.cancel() }
        return retain(lease)
    }

    private func retain(_ lease: ArtifactLeaseHandle) -> ArtifactLeaseHandle {
        guard let artifact = try? lease.metadata() else { return lease }
        let now = Int64(Date().timeIntervalSince1970)
        retained = retained.filter { key, handle in
            guard handle !== lease else { return false }
            return ((try? handle.metadata())?.retainedUntil ?? 0) > now && key != artifact.id
        }
        retained[artifact.id] = lease
        return lease
    }

    private func record(_ stickerId: String, _ action: UsageAction) async throws {
        _ = try await session.record(stickerId, action: action)
    }

    private func run(_ body: () async throws -> Void) async throws {
        guard !busy else { throw AppFailure(.errorBusy) }
        busy = true
        defer { busy = false }
        try await body()
    }

    private static func anchorView() -> NSView? {
        NSApp.keyWindow?.contentView
    }

    private static func askDestination(name: String) async throws -> URL? {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        return panel.runModal() == .OK ? panel.url : nil
    }

    private static func replace(source: URL, destination: URL) throws {
        let manager = FileManager.default
        if manager.fileExists(atPath: destination.path) {
            try manager.removeItem(at: destination)
        }
        try manager.copyItem(at: source, to: destination)
    }

    /// The exported file already carries the final bytes, so the pasteboard
    /// receives them unchanged: re-encoding here would drop animation.
    private static func write(artifact: ExportArtifact, representation: Representation, to pasteboard: NSPasteboard) throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: artifact.path))
        let item = NSPasteboardItem()
        item.setString(URL(fileURLWithPath: artifact.path).absoluteString, forType: .fileURL)
        if let type = pasteboardType(for: artifact.mime, representation: representation) {
            item.setData(data, forType: type)
        }
        pasteboard.clearContents()
        guard pasteboard.writeObjects([item]) else {
            throw AppFailure(.errorIo)
        }
    }

    private static func pasteboardType(for mime: String, representation: Representation) -> NSPasteboard.PasteboardType? {
        switch mime {
        case "image/png": return .png
        case "image/jpeg": return NSPasteboard.PasteboardType("public.jpeg")
        case "image/gif": return NSPasteboard.PasteboardType("com.compuserve.gif")
        case "image/webp": return NSPasteboard.PasteboardType("org.webmproject.webp")
        default:
            return representation == .image ? nil : NSPasteboard.PasteboardType("public.data")
        }
    }
}
