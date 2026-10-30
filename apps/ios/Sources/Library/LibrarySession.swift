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
    private struct BrowserReference {
        weak var browser: LibraryBrowser?
    }
    private(set) var phase: LibraryPhase = .idle
    private(set) var statistics: SpaceStatistics?
    private(set) var browser: LibraryBrowser?
    private(set) var thumbnails: ThumbnailStore?
    private(set) var identity: LibraryIdentity?
    private(set) var queryError: String?
    private(set) var library: LibraryHandle?
    private(set) var imports: ImportCoordinator?
    private(set) var organize: OrganizeModel?
    private(set) var trash: TrashModel?
    private(set) var outputs: OutputCoordinator?
    private(set) var sync: SyncModel?
    private(set) var backup: BackupModel?
    private(set) var telegram: TelegramModel?
    private(set) var restoreEpoch: UInt64 = 0
    private(set) var isRestoring = false
    let outputPreferences = OutputPreferences()
    let feedback = AppFeedback()

    @ObservationIgnored private let paths: AppPaths?
    @ObservationIgnored private var subscription: SubscriptionHandle?
    @ObservationIgnored private var feed: Task<Void, Never>?
    @ObservationIgnored private var refreshGeneration: UInt64?
    @ObservationIgnored private var refreshPending = false
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var details: [DetailReference] = []
    @ObservationIgnored private var limits: ResourceConfiguration?
    @ObservationIgnored private var groupBrowsers: [BrowserReference] = []
    @ObservationIgnored private var opening: Task<Void, Never>?

    init(paths: AppPaths? = nil) {
        self.paths = paths
    }

    func start() async {
        if let opening {
            await opening.value
            return
        }
        guard library == nil, phase != .closing else { return }
        let operation = Task { await self.open() }
        opening = operation
        await operation.value
        opening = nil
    }

    private func open() async {
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
            organize = OrganizeModel(library: handle)
            trash = TrashModel(library: handle)
            outputs = OutputCoordinator(library: handle, feedback: feedback)
            sync = SyncModel(library: handle)
            telegram = TelegramModel(library: handle, feedback: feedback)
            if let backup {
                backup.replaceLibrary(handle)
            } else {
                backup = BackupModel(
                    library: handle, feedback: feedback,
                    beforeRestore: { [weak self] in
                        guard let self else { throw BridgeError.Failure(code: .closed, detail: "") }
                        try await self.prepareRestore()
                    }, didRestore: { [weak self] in await self?.reconnectAfterRestore() })
            }
            thumbnails = ThumbnailStore(library: handle)
            // No transfer is active before the session exposes its coordinator.
            try await ImportFileIO.removeAbandonedPhotoCopies()
            imports = ImportCoordinator(library: handle, maximumBytes: configuration.limits.maxFileBytes) {
                [weak self] in
                if let self, let imports = self.imports { self.reportImport(imports) }
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
            await organize?.refresh()
            await sync?.start()
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
        await refreshStatistics()
        guard phase == .ready else { return }
        await browser?.refresh()
    }

    func refreshStatistics() async {
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
            } catch {
                guard generation == current else { return }
                queryError = BridgeErrors.describe(error)
            }
        } while refreshPending && generation == current && phase == .ready
    }

    func shutdown(preserveBackup: Bool = false) async {
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
            for reference in groupBrowsers { reference.browser?.stop() }
            groupBrowsers = []
            await organize?.close()
            await trash?.close()
            await outputs?.close()
            await sync?.close()
            await telegram?.close()
            if !preserveBackup {
                await backup?.close()
                backup = nil
            }
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
            organize = nil
            trash = nil
            outputs = nil
            sync = nil
            telegram = nil
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

    private func prepareRestore() async throws {
        guard opening == nil, phase == .ready, imports?.isRunning != true, outputs?.isBusy != true,
            telegram?.isBusy != true, browser?.isMutating != true,
            !groupBrowsers.contains(where: { $0.browser?.isMutating == true }),
            organize?.isMutating != true, trash?.isMutating != true
        else {
            throw BridgeError.Failure(code: .busy, detail: "")
        }
        isRestoring = true
        await sync?.beginMaintenance()
        browser?.stop()
        for value in groupBrowsers { value.browser?.stop() }
        let active = details.compactMap(\.model)
        for model in active { await model.close() }
        await thumbnails?.clearCache()
    }

    private func reportImport(_ imports: ImportCoordinator) {
        let items = imports.items
        let failed = items.filter {
            switch $0.status {
            case .failed, .restoreRequired: true
            default: false
            }
        }.count
        let added = imports.added
        let latest = items.last { $0.status == .created || $0.status == .reused }?.stickerID
        if items.contains(where: { $0.status == .cancelled }) {
            feedback.post(String(localized: "Import stopped. \(added) stickers added."), kind: .warning)
        } else if failed > 0 {
            feedback.post(String(localized: "\(added) added, \(failed) failed"), kind: .warning, action: .viewImport)
        } else if added == 0 {
            feedback.post(String(localized: "All selected stickers already exist"), action: latest.map { .viewImported($0) })
        } else if imports.reused > 0 {
            feedback.post(
                String(localized: "\(added) added, \(imports.reused) already exist"),
                action: latest.map { .viewImported($0) })
        } else {
            feedback.post(String(localized: "\(added) stickers added"), action: latest.map { .viewImported($0) })
        }
    }

    private func reconnectAfterRestore() async {
        await shutdown(preserveBackup: true)
        if library == nil { await start() }
        isRestoring = false
        restoreEpoch &+= 1
    }

    func makeGroupBrowser(for route: GroupRoute) -> LibraryBrowser? {
        guard phase == .ready, let library, organize?.name(for: route) != nil else { return nil }
        groupBrowsers.removeAll { $0.browser == nil }
        let browser = LibraryBrowser(library: library, scope: route.filter)
        groupBrowsers.append(BrowserReference(browser: browser))
        return browser
    }

    func makeDetail(for route: StickerDetailRoute) -> StickerDetailModel? {
        guard phase == .ready, let library, let thumbnails, let limits else { return nil }
        details.removeAll { $0.model == nil }
        let seed = ([browser] + groupBrowsers.map(\.browser)).compactMap { $0 }
            .flatMap(\.items).first { $0.id == route.id && $0.sticker.lifecycle.generation == route.generation }
        let model = StickerDetailModel(
            route: route, library: library, thumbnails: thumbnails, limits: limits, seed: seed)
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
        groupBrowsers.removeAll { $0.browser == nil }
        switch event {
        case .thumbnailChanged(_, let id):
            await browser?.refreshThumbnail(id)
            for reference in groupBrowsers { await reference.browser?.refreshThumbnail(id) }
        case .collectionsChanged:
            await browser?.loadCollections()
            browser?.libraryChanged()
            refreshDetails()
            organize?.scheduleRefresh()
            for reference in groupBrowsers { reference.browser?.libraryChanged() }
        case .stickerChanged(_, let id):
            browser?.libraryChanged()
            refreshDetails(id: id)
            organize?.scheduleRefresh()
            for reference in groupBrowsers { reference.browser?.libraryChanged() }
            await refreshStatistics()
        case .tagsChanged:
            browser?.libraryChanged()
            refreshDetails()
            organize?.scheduleRefresh()
            for reference in groupBrowsers { reference.browser?.libraryChanged() }
        case .usageChanged:
            if browser?.filter == .recentlyUsed { browser?.libraryChanged() }
        case .reloadRequired:
            await thumbnails?.clearCache()
            browser?.invalidateThumbnails()
            await browser?.loadCollections()
            browser?.libraryChanged()
            refreshDetails()
            organize?.scheduleRefresh()
            for reference in groupBrowsers {
                reference.browser?.invalidateThumbnails()
                reference.browser?.libraryChanged()
            }
            await refreshStatistics()
        case .closed:
            break
        }
    }
}
