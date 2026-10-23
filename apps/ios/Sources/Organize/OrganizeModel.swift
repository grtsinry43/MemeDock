import Foundation
import MemeDockBridge
import Observation

@MainActor
@Observable
final class OrganizeModel {
    private(set) var collections: [CollectionSummary] = []
    private(set) var tags: [TagMetadata] = []
    private(set) var isLoading = false
    private(set) var error: String?
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var collectionTask: CollectionSummariesTask?
    @ObservationIgnored private var tagTask: TagsTask?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var closed = false

    init(library: LibraryHandle) { self.library = library }

    deinit {
        work?.cancel()
        _ = collectionTask?.cancel()
        _ = tagTask?.cancel()
    }

    func refresh() async {
        scheduleRefresh()
        await work?.value
    }

    func scheduleRefresh() {
        guard !closed else { return }
        cancel()
        let current = generation
        isLoading = true
        work = Task { [weak self] in
            guard let self else { return }
            defer {
                if generation == current {
                    isLoading = false
                    collectionTask = nil
                    tagTask = nil
                }
            }
            do {
                let collectionQuery = try library.collectionSummaries()
                collectionTask = collectionQuery
                let values = try await collectionQuery.awaitResult()
                try Task.checkCancellation()
                guard generation == current, !closed else { return }
                let tagQuery = try library.tags(deleted: false)
                tagTask = tagQuery
                let tagValues = try await tagQuery.awaitResult()
                guard generation == current, !closed, !Task.isCancelled else { return }
                collections = values
                tags = tagValues
                error = nil
            } catch {
                guard generation == current, !closed, !Task.isCancelled else { return }
                self.error = BridgeErrors.browseDescription(error)
            }
        }
    }

    func name(for route: GroupRoute) -> String? {
        switch route.filter {
        case .collection(let id):
            collections.first { $0.collection.id == id && $0.collection.lifecycle.generation == route.generation }?.collection.name
        case .tag(let id):
            tags.first { $0.id == id && $0.lifecycle.generation == route.generation }?.name
        default: nil
        }
    }

    func waitUntilSettled() async { await work?.value }

    func close() async {
        closed = true
        let operation = work
        cancel()
        await operation?.value
        collections = []
        tags = []
        isLoading = false
    }

    private func cancel() {
        generation &+= 1
        work?.cancel()
        _ = collectionTask?.cancel()
        _ = tagTask?.cancel()
    }
}
