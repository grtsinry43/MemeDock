import CoreGraphics
import Foundation
import MemeDockBridge
import Observation

struct StickerDetailRoute: Hashable {
    let id: String
    let generation: Int64
    let includesDeleted: Bool

    init(_ item: LibraryItem, includesDeleted: Bool = false) {
        id = item.id
        generation = item.sticker.lifecycle.generation
        self.includesDeleted = includesDeleted
    }
}

@MainActor
@Observable
final class StickerDetailModel {
    enum Phase: Equatable {
        case idle, loading, ready, closed
        case failed(String)
        case unavailable(String)
    }

    let route: StickerDetailRoute
    let playback = AnimatedPlayback()
    private(set) var phase: Phase = .idle
    private(set) var detail: StickerDetail?
    private(set) var placeholder: CGImage?
    private(set) var originalError: String?
    private(set) var isManaging = false
    private(set) var managementError: String?
    private(set) var restoreSuggestions: RestoreSuggestions?
    @ObservationIgnored private var management: Task<Bool, Never>?
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let thumbnails: ThumbnailStore
    @ObservationIgnored private let limits: ResourceConfiguration
    @ObservationIgnored private let seed: LibraryItem?
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var scheduled: Task<Void, Never>?
    @ObservationIgnored private var query: StickerDetailTask?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var pixels = 1280
    @ObservationIgnored private var decodedPixels = 0
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var reduceMotion = false

    var canControlPlayback: Bool {
        if case .failed = playback.state { return false }
        return originalError == nil && playback.isAnimated
    }

    func purge() async -> Bool {
        guard let sticker = detail?.sticker, sticker.lifecycle.deletedAt != nil else { return false }
        let revision = sticker.lifecycle.revision
        return await manage {
            _ = try await self.library.purgeSticker(
                target: EntityReference(
                    id: sticker.id,
                    generation: sticker.lifecycle.generation), deletedRevision: revision
            ).awaitResult()
        }
    }

    init(
        route: StickerDetailRoute, library: LibraryHandle, thumbnails: ThumbnailStore,
        limits: ResourceConfiguration, seed: LibraryItem? = nil
    ) {
        self.route = route
        self.library = library
        self.thumbnails = thumbnails
        self.limits = limits
        self.seed = seed
    }

    deinit {
        work?.cancel()
        scheduled?.cancel()
        _ = query?.cancel()
        let ownedPlayback = playback
        Task { @MainActor in await ownedPlayback.shutdown() }
    }

    func load(pixels: Int) async {
        scheduled?.cancel()
        beginLoad(pixels: pixels)
        let current = generation
        let operation = work
        await withTaskCancellationHandler {
            await operation?.value
        } onCancel: {
            Task { @MainActor [weak self] in
                guard let self, self.generation == current else { return }
                self.suspend()
            }
        }
    }

    func scheduleReload() {
        guard !closed, phase != .idle else { return }
        generation &+= 1
        _ = query?.cancel()
        work?.cancel()
        scheduled?.cancel()
        scheduled = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(100)) } catch { return }
            guard let self, !self.closed, !Task.isCancelled else { return }
            self.beginLoad(pixels: self.pixels)
        }
    }

    func waitUntilLoaded() async {
        await scheduled?.value
        await work?.value
    }

    func setPresentation(active: Bool, reduceMotion: Bool) {
        self.reduceMotion = reduceMotion
        playback.setPresentation(active: active, reduceMotion: reduceMotion)
    }

    func suspend() {
        generation &+= 1
        scheduled?.cancel()
        _ = query?.cancel()
        query = nil
        work?.cancel()
        playback.setPresentation(active: false, reduceMotion: reduceMotion)
    }

    func update(title: String? = nil, note: String? = nil, starred: Bool? = nil) async -> Bool {
        await manage {
            _ = try await self.library.patchSticker(
                target: EntityReference(id: self.route.id, generation: self.route.generation),
                edit: StickerEdit(title: title, note: note, starred: starred)
            ).awaitResult()
        }
    }

    func organize(collection: EntityReference?, tags: [EntityReference]) async -> Bool {
        await manage {
            _ = try await self.library.setStickerOrganization(
                target: EntityReference(id: self.route.id, generation: self.route.generation),
                changeCollection: true, collection: collection, tags: tags
            ).awaitResult()
        }
    }

    func delete() async -> Bool {
        await manage {
            _ = try await self.library.deleteSticker(
                target: EntityReference(id: self.route.id, generation: self.route.generation)
            ).awaitResult()
        }
    }

    func loadRestoreSuggestions() async {
        do {
            let suggestions = try await library.restoreSuggestions(id: route.id).awaitResult()
            if !closed { restoreSuggestions = suggestions }
        } catch {
            if !closed { managementError = BridgeErrors.managementDescription(error) }
        }
    }

    func restore() async -> Bool {
        guard let detail, detail.sticker.lifecycle.deletedAt != nil else { return false }
        let revision = detail.sticker.lifecycle.revision
        return await manage {
            _ = try await self.library.restoreSticker(
                target: EntityReference(id: self.route.id, generation: self.route.generation),
                deletedRevision: revision
            ).awaitResult()
        }
    }

    func clearManagementError() { managementError = nil }

    private func manage(_ operation: @escaping @MainActor () async throws -> Void) async -> Bool {
        guard !closed, !isManaging else { return false }
        isManaging = true
        managementError = nil
        management = Task {
            defer { self.isManaging = false }
            do {
                try await operation()
                guard !self.closed else { return false }
                await self.load(pixels: self.pixels)
                return true
            } catch {
                if !self.closed { self.managementError = BridgeErrors.managementDescription(error) }
                return false
            }
        }
        return await management?.value ?? false
    }

    func close() async {
        closed = true
        _ = await management?.value
        let loading = work
        suspend()
        await loading?.value
        await playback.shutdown()
        detail = nil
        placeholder = nil
        originalError = nil
        phase = .closed
    }

    private func beginLoad(pixels: Int) {
        guard !closed else { return }
        generation &+= 1
        let current = generation
        _ = query?.cancel()
        work?.cancel()
        self.pixels = max(1, min(pixels, 1280))
        let targetPixels = self.pixels
        phase = .loading
        work = Task { [weak self] in
            guard let self, !closed, generation == current else { return }
            defer { if generation == current { query = nil } }
            do {
                if placeholder == nil, let seed {
                    do {
                        let thumbnail = try await thumbnails.image(for: seed, pixels: min(targetPixels, 512))
                        guard generation == current, !closed, !Task.isCancelled else { return }
                        placeholder = thumbnail
                    } catch {
                        try Task.checkCancellation()
                        // A missing thumbnail must not prevent loading the actual original.
                    }
                }
                try Task.checkCancellation()
                let task = try library.stickerDetail(id: route.id)
                query = task
                let value = try await task.awaitResult()
                guard generation == current, !closed, !Task.isCancelled else { return }
                guard value.sticker.lifecycle.generation == route.generation,
                    value.sticker.lifecycle.deletedAt == nil || route.includesDeleted,
                    value.sticker.lifecycle.purgedAt == nil
                else {
                    detail = nil
                    playback.reset()
                    phase = .unavailable(
                        String(
                            localized: "This sticker was deleted or replaced. Return to the library to select it again."
                        ))
                    return
                }
                let decodeFailed: Bool
                if case .failed = playback.state { decodeFailed = true } else { decodeFailed = false }
                let needsDecode =
                    decodeFailed || detail?.asset != value.asset || detail?.originalPath != value.originalPath
                    || !playback.hasConfiguration || decodedPixels != targetPixels
                detail = value
                originalError = nil
                if let path = value.originalPath {
                    if needsDecode {
                        await playback.load(path: path, asset: value.asset, limits: limits, pixels: targetPixels)
                        guard generation == current, !closed, !Task.isCancelled else { return }
                        decodedPixels = targetPixels
                    }
                } else {
                    playback.reset()
                    originalError = BridgeErrors.originalDescription(value.originalError)
                }
                phase = .ready
            } catch {
                guard generation == current, !closed, !Task.isCancelled else { return }
                playback.reset()
                if case BridgeError.Failure(let code, _) = error, code == .notFound || code == .entityDeleted {
                    detail = nil
                    phase = .unavailable(BridgeErrors.browseDescription(error))
                } else {
                    phase = .failed(BridgeErrors.browseDescription(error))
                }
            }
        }
    }
}
