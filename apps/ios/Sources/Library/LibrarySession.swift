import Foundation
import MemeDockBridge
import Observation

enum LibraryPhase: Equatable {
    case idle
    case opening
    case ready
    case closing
    case closed
    case failed(String)
}

@MainActor
@Observable
final class LibrarySession {
    private struct DetailReference {
        weak var model: StickerDetailModel?
    }
    private(set) var phase: LibraryPhase = .idle
    private(set) var statistics: SpaceStatistics?
    private(set) var browser: LibraryBrowser?
    private(set) var thumbnails: ThumbnailStore?
    private(set) var identity: LibraryIdentity?
    private(set) var queryError: String?
    private(set) var library: LibraryHandle?
    private(set) var imports: ImportCoordinator?

    @ObservationIgnored private let paths: AppPaths?
    @ObservationIgnored private var subscription: SubscriptionHandle?
    @ObservationIgnored private var feed: Task<Void, Never>?
    @ObservationIgnored private var refreshGeneration: UInt64?
    @ObservationIgnored private var refreshPending = false
    @ObservationIgnored private var generation: UInt64 = 0

    @ObservationIgnored private var details: [DetailReference] = []
    @ObservationIgnored private var limits: ResourceConfiguration?

    init(paths: AppPaths? = nil) {
        self.paths = paths
    }

    func start() async {
        guard library == nil, phase != .opening, phase != .closing else { return }
        phase = .opening
        generation &+= 1
        let current = generation
        do {
            let paths = try paths ?? AppPaths.standard()
            let configuration = try paths.configuration()
            limits = configuration.limits
            let handle = try await openLibrary(configuration: configuration)
            library = handle
            browser = LibraryBrowser(library: handle)
            thumbnails = ThumbnailStore(library: handle)
            // No transfer is active before the session exposes its coordinator.
            try await ImportFileIO.removeAbandonedPhotoCopies()
            imports = ImportCoordinator(library: handle, maximumBytes: configuration.limits.maxFileBytes) { [weak self] in
                await self?.refresh()
            }
            identity = handle.identity()
            let stream = try handle.subscribe()
            subscription = stream
            phase = .ready
            feed = Task { [weak self] in
                do {
                    while !Task.isCancelled {
                        let event = try await stream.next()
                        guard let self, self.generation == current else { return }
                        if event == .closed { return }
                        await self.apply(event)
                    }
                } catch {
                    guard !Task.isCancelled, let self, self.generation == current else { return }
                    self.queryError = BridgeErrors.describe(error)
                }
            }
            await browser?.loadCollections()
            await refresh()
        } catch {
            let detail = BridgeErrors.describe(error)
            if library != nil {
                await shutdown()
                if library != nil { return }
            }
            phase = .failed(detail)
        }
    }

    func retry() async {
        if library != nil {
            await shutdown()
            guard library == nil else { return }
        }
        await start()
    }

    func refresh() async {
        guard let handle = library, phase == .ready else { return }
        let current = generation
        if refreshGeneration == current {
            refreshPending = true
            return
        }
        refreshGeneration = current
        defer {
            if refreshGeneration == current { refreshGeneration = nil }
        }
        repeat {
            refreshPending = false
            do {
                let statisticsTask = try handle.spaceStatistics()
                let totals = try await statisticsTask.awaitResult()
                guard generation == current, phase == .ready else { return }
                statistics = totals
                queryError = nil
                await browser?.refresh()
            } catch {
                guard generation == current else { return }
                queryError = BridgeErrors.describe(error)
            }
        } while refreshPending && generation == current && phase == .ready
    }

    func shutdown() async {
        guard phase != .closing else { return }
        // Only the opening operation can clean up its partially opened handle.
        guard phase != .opening || library != nil else { return }
        guard let handle = library else {
            phase = .closed
            return
        }
        phase = .closing
        generation &+= 1
        refreshPending = false
        feed?.cancel()
        do {
            browser?.stop()
            let detailModels = details.compactMap(\.model)
            details = []
            for model in detailModels { await model.close() }
            await imports?.stopAndWait()
            try subscription?.unsubscribe()
            await feed?.value
            feed = nil
            subscription = nil
            await thumbnails?.shutdown()
            try await handle.shutdown()
            library = nil
            imports = nil
            browser = nil
            thumbnails = nil
            statistics = nil
            identity = nil
            queryError = nil
            limits = nil
            phase = .closed
        } catch {
            // Keep ownership when shutdown fails; never open another instance.
            phase = .failed(BridgeErrors.describe(error))
        }
    }

    func makeDetail(for route: StickerDetailRoute) -> StickerDetailModel? {
        guard phase == .ready, let library, let thumbnails, let limits else { return nil }
        details.removeAll { $0.model == nil }
        let seed = browser?.items.first { $0.id == route.id && $0.sticker.lifecycle.generation == route.generation }
        let model = StickerDetailModel(route: route, library: library, thumbnails: thumbnails, limits: limits, seed: seed)
        details.append(DetailReference(model: model))
        return model
    }

    private func refreshDetails(id: String? = nil) {
        details.removeAll { $0.model == nil }
        for model in details.compactMap(\.model) where id == nil || model.route.id == id {
            model.scheduleReload()
        }
    }

    private func apply(_ event: MemeDockBridge.Notification) async {
        switch event {
        case .thumbnailChanged(_, let id):
            await browser?.refreshThumbnail(id)
        case .collectionsChanged:
            await browser?.loadCollections()
            browser?.libraryChanged()
            refreshDetails()
        case .stickerChanged(_, let id):
            browser?.libraryChanged()
            refreshDetails(id: id)
        case .tagsChanged:
            browser?.libraryChanged()
            refreshDetails()
        case .usageChanged:
            if browser?.filter == .recentlyUsed { browser?.libraryChanged() }
        case .reloadRequired:
            await thumbnails?.clearCache()
            browser?.invalidateThumbnails()
            await browser?.loadCollections()
            browser?.libraryChanged()
            refreshDetails()
        case .closed:
            break
        }
    }
}
