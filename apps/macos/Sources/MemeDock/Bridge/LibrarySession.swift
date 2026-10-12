import Combine
import Foundation
import MemeDockBridge

enum LibraryPhase: Equatable {
    case opening
    case ready
    case failed(String)
    case closed
}

/// One notification from the core, forwarded to whichever model cares.
enum SessionEvent: Equatable {
    case collections
    case tags
    case sticker(String)
    case thumbnail(String)
    case usage(String)
    case reload
}

/// Owns the single core instance: opening, notification fan-out, library-wide
/// statistics, and shutdown. Exactly one `next()` reader is kept alive.
@MainActor
final class LibrarySession: ObservableObject {
    @Published private(set) var phase: LibraryPhase = .opening
    @Published private(set) var statistics: SpaceStatistics?
    @Published private(set) var identity: LibraryIdentity?

    /// Every notification from the core, in order. A published property would
    /// drop two identical events in a row, and a missed one leaves the sidebar
    /// showing stale names.
    var events: AnyPublisher<SessionEvent, Never> { changes.eraseToAnyPublisher() }

    private let changes = PassthroughSubject<SessionEvent, Never>()

    private(set) var library: LibraryHandle?
    private(set) var limits: ResourceConfiguration?
    let paths: AppPaths

    private var subscription: SubscriptionHandle?
    private var feed: Task<Void, Never>?

    init(paths: AppPaths) {
        self.paths = paths
    }

    func start() async {
        guard library == nil else { return }
        phase = .opening
        do {
            try paths.prepare()
            let limits = try defaultResourceConfiguration()
            self.limits = limits
            let configuration = LibraryConfiguration(
                dataDir: paths.data.path,
                cacheDir: paths.cache.path,
                exportDir: paths.export.path,
                limits: limits
            )
            let library = try await openLibrary(configuration: configuration)
            self.library = library
            self.identity = library.identity()
            subscribe(to: library)
            phase = .ready
            await refreshStatistics()
        } catch {
            phase = .failed(describe(error))
        }
    }

    func shutdown() async {
        feed?.cancel()
        feed = nil
        if let subscription {
            _ = try? subscription.unsubscribe()
        }
        subscription = nil
        guard let library else {
            phase = .closed
            return
        }
        _ = try? await library.shutdown()
        self.library = nil
        phase = .closed
    }

    /// A replacement restore leaves a different library at the same path, so
    /// the core instance is closed and opened again - the same reconnect the
    /// Linux client performs after a replacement.
    func reopen() async {
        await shutdown()
        await start()
        announceReload()
    }

    /// Asks every model to rebuild from the library. A restore changes things
    /// the core does not report one by one, so the readers are told directly.
    func announceReload() {
        changes.send(.reload)
    }

    func refreshStatistics() async {
        guard let library, let task = try? library.spaceStatistics() else { return }
        statistics = try? await task.awaitResult()
    }

    /// A checkpoint is a consistent copy of the database; the platform only
    /// reports where it landed.
    func createCheckpoint() async -> Result<String, Error> {
        guard let library else { return .failure(AppFailure(.errorClosed)) }
        do {
            let task = try library.createCheckpoint()
            return .success(try await task.awaitResult())
        } catch {
            return .failure(error)
        }
    }

    func cleanExportArtifacts() async -> Result<UInt64, Error> {
        guard let library else { return .failure(AppFailure(.errorClosed)) }
        do {
            let task = try library.cleanExportArtifacts()
            return .success(try await task.awaitResult())
        } catch {
            return .failure(error)
        }
    }

    private func subscribe(to library: LibraryHandle) {
        do {
            let handle = try library.subscribe()
            subscription = handle
            feed = Task { [weak self] in
                while !Task.isCancelled {
                    let next: MemeDockBridge.Notification
                    do {
                        next = try await handle.next()
                    } catch {
                        return
                    }
                    guard let self else { return }
                    switch next {
                    case .closed:
                        return
                    case .collectionsChanged:
                        self.changes.send(.collections)
                    case .tagsChanged:
                        self.changes.send(.tags)
                    case let .stickerChanged(_, stickerId):
                        self.changes.send(.sticker(stickerId))
                    case let .thumbnailChanged(_, stickerId):
                        self.changes.send(.thumbnail(stickerId))
                    case let .usageChanged(_, stickerId):
                        self.changes.send(.usage(stickerId))
                    case .reloadRequired:
                        self.changes.send(.reload)
                    }
                }
            }
        } catch {
            phase = .failed(describe(error))
        }
    }
}
