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
    @Published private(set) var event: SessionEvent?

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
                        self.event = .collections
                    case .tagsChanged:
                        self.event = .tags
                    case let .stickerChanged(_, stickerId):
                        self.event = .sticker(stickerId)
                    case let .thumbnailChanged(_, stickerId):
                        self.event = .thumbnail(stickerId)
                    case let .usageChanged(_, stickerId):
                        self.event = .usage(stickerId)
                    case .reloadRequired:
                        self.event = .reload
                    }
                }
            }
        } catch {
            phase = .failed(describe(error))
        }
    }
}
