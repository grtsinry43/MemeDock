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
    /// A preset picked in the detail pane for this run of the app. It replaces
    /// the copy and share defaults while set; `nil` follows the settings.
    @Published var override: ExportPreset?

    /// What copying an image or a file hands over right now.
    var copyPreset: ExportPreset { override ?? preferences.copyPreset }
    /// What sharing and saving hand over right now.
    var sharePreset: ExportPreset { override ?? preferences.sharePreset }
    /// What a drag hands over; the detail pane's temporary choice never
    /// changes it.
    var dragPreset: ExportPreset { preferences.dragPreset }

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
    }

    func copyImage(_ item: StickerItem, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.copyPreset, firstFrame: firstFrame)
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
            let lease = try await self.export(item, preset: self.copyPreset, firstFrame: false)
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
            let lease = try await self.export(item, preset: self.sharePreset, firstFrame: firstFrame)
            let artifact = try lease.metadata()
            guard let destination = try await Self.askDestination(name: artifact.fileName) else { return }
            try Self.replace(source: URL(fileURLWithPath: artifact.path), destination: destination)
            try await self.record(item.id, .exportSaved)
            self.status.show(.saved)
        }
    }

    func share(_ item: StickerItem, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: self.sharePreset, firstFrame: firstFrame)
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
        guard let library = session.library else { return NSItemProvider() }
        var outcome = exportedFile(
            library: library,
            id: item.id,
            animated: item.animated,
            preset: dragPreset,
            timeout: 2
        )
        // A preset conversion can fail where the stored file is still usable.
        if case .success? = outcome {} else {
            outcome = exportedFile(
                library: library,
                id: item.id,
                animated: item.animated,
                preset: .original,
                timeout: 1
            )
        }
        let fileURL: URL
        switch outcome {
        case let .success(url)?:
            fileURL = url
        case let .failure(error)?:
            status.show(error)
            return NSItemProvider()
        case nil:
            status.show(L10n.text(.errorTimeout), isFailure: true)
            return NSItemProvider()
        }
        return NSItemProvider(contentsOf: fileURL) ?? NSItemProvider()
    }

    private func exportedFile(
        library: LibraryHandle,
        id: String,
        animated: Bool,
        preset: ExportPreset,
        timeout: TimeInterval
    ) -> Result<URL, Error>? {
        let resolution = StickerExport.resolve(preset: preset, animated: animated, firstFrame: false)
        let options = StickerExport.options(resolution.preset, animation: resolution.animation)
        let export = DragExport(task: Task.detached(priority: .userInitiated) {
            let lease = try await library.export(id: id, options: options).awaitResult()
            let artifact = try lease.metadata()
            return URL(fileURLWithPath: artifact.path)
        })
        return export.wait(timeout: timeout)
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
        let lease = try await session.requireLibrary().export(id: item.id, options: options).awaitResult()
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
