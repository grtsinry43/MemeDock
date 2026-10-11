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

    private let session: LibrarySession
    private let clipboard: ClipboardGuard
    private let status: StatusCenter
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
            let lease = try await self.export(item, preset: .compatiblePng, firstFrame: firstFrame)
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
            let lease = try await self.exportOriginal(item)
            let artifact = try lease.metadata()
            try await self.clipboard.install(lease: lease) { pasteboard in
                try Self.write(artifact: artifact, representation: .file, to: pasteboard)
            }
            try await self.record(item.id, .copyFile)
            self.status.show(.fileCopied)
        }
    }

    func save(_ item: StickerItem, preset: ExportPreset, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: preset, firstFrame: firstFrame)
            let artifact = try lease.metadata()
            guard let destination = try await Self.askDestination(name: artifact.fileName) else { return }
            try Self.replace(source: URL(fileURLWithPath: artifact.path), destination: destination)
            try await self.record(item.id, .exportSaved)
            self.status.show(.saved)
        }
    }

    func share(_ item: StickerItem, preset: ExportPreset, firstFrame: Bool = false) async throws {
        try await run {
            let lease = try await self.export(item, preset: preset, firstFrame: firstFrame)
            let artifact = try lease.metadata()
            let url = URL(fileURLWithPath: artifact.path)
            guard let view = Self.anchorView() else { throw AppFailure(.errorInternal) }
            let picker = NSSharingServicePicker(items: [url])
            picker.show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
            try await self.record(item.id, .shareLaunched)
            self.status.show(.shareLaunched)
        }
    }

    /// Prepares a file for a drag session and returns a provider that hands the
    /// file URL (plus its bytes) to the drop target.
    func dragProvider(for item: StickerItem, firstFrame: Bool = false) async throws -> NSItemProvider {
        let lease = try await session
            .requireLibrary()
            .exportOriginal(id: item.id)
            .awaitResult()
        guard let artifact = try? lease.metadata() else { throw AppFailure(.errorInternal) }
        return Self.provider(artifact: artifact)
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

    private func exportOriginal(_ item: StickerItem) async throws -> ArtifactLeaseHandle {
        let lease = try await session.requireLibrary().exportOriginal(id: item.id).awaitResult()
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

    private static func provider(artifact: ExportArtifact) -> NSItemProvider {
        let provider = NSItemProvider()
        let url = URL(fileURLWithPath: artifact.path)
        let uti = pasteboardType(for: artifact.mime, representation: .file)?.rawValue ?? "public.data"
        provider.registerFileRepresentation(
            forTypeIdentifier: uti,
            fileOptions: [],
            visibility: .all
        ) { completion in
            completion(url, false, nil)
            return nil
        }
        if let type = pasteboardType(for: artifact.mime, representation: .image) {
            provider.registerDataRepresentation(
                forTypeIdentifier: type.rawValue,
                visibility: .all
            ) { completion in
                completion(try? Data(contentsOf: url), nil)
                return nil
            }
        }
        return provider
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
