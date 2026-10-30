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
    private(set) var isMutating = false
    private(set) var mutationError: String?
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var collectionTask: CollectionSummariesTask?
    @ObservationIgnored private var tagTask: TagsTask?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var mutation: Task<Bool, Never>?

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
        guard !closed, !isMutating else { return }
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
            collections.first { $0.collection.id == id && $0.collection.lifecycle.generation == route.generation }?
                .collection.name
        case .tag(let id):
            tags.first { $0.id == id && $0.lifecycle.generation == route.generation }?.name
        default: nil
        }
    }

    func waitUntilSettled() async { await work?.value }

    func save(name: String, editing: GroupEditRequest) async -> Bool {
        await mutate {
            switch editing {
            case .newCollection:
                _ = try await self.library.createCollection(name: name, before: nil).awaitResult()
            case .newTag:
                _ = try await self.library.createTag(name: name).awaitResult()
            case .rename(let route):
                guard let target = self.reference(for: route) else { throw CancellationError() }
                switch route.filter {
                case .collection:
                    _ = try await self.library.renameCollection(target: target, name: name).awaitResult()
                case .tag:
                    _ = try await self.library.renameTag(target: target, name: name).awaitResult()
                default: throw CancellationError()
                }
            }
        }
    }

    func delete(_ route: GroupRoute) async -> Bool {
        await mutate {
            guard let target = self.reference(for: route) else { throw CancellationError() }
            switch route.filter {
            case .collection:
                _ = try await self.library.deleteCollection(target: target).awaitResult()
            case .tag:
                _ = try await self.library.deleteTag(target: target).awaitResult()
            default: throw CancellationError()
            }
        }
    }

    func moveCollection(_ route: GroupRoute, before: String?) async -> Bool {
        guard !closed, !isMutating, case .collection(let id) = route.filter,
            let index = collections.firstIndex(where: { $0.collection.id == id }), id != before
        else { return false }
        let previous = collections
        let moving = collections.remove(at: index)
        let destination =
            before.flatMap { next in collections.firstIndex { $0.collection.id == next } } ?? collections.endIndex
        collections.insert(moving, at: destination)
        let success = await mutate {
            guard case .collection = route.filter, let target = self.reference(for: route) else {
                throw CancellationError()
            }
            _ = try await self.library.moveCollection(target: target, before: before).awaitResult()
        }
        if !success { collections = previous }
        return success
    }

    func moveSticker(_ sticker: StickerMetadata, in route: GroupRoute, before: String?) async -> Bool {
        await mutate {
            guard case .collection = route.filter, let collection = self.reference(for: route) else {
                throw CancellationError()
            }
            try await self.library.moveCollectionItem(
                collection: collection,
                sticker: EntityReference(id: sticker.id, generation: sticker.lifecycle.generation), before: before
            ).awaitResult()
        }
    }

    func coverItem(for summary: CollectionSummary) async throws -> LibraryItem? {
        guard !closed, let id = summary.cover else { return nil }
        let value = try await library.stickerDetail(id: id).awaitResult()
        try Task.checkCancellation()
        guard !closed, value.sticker.lifecycle.deletedAt == nil, value.sticker.lifecycle.purgedAt == nil else {
            return nil
        }
        return LibraryItem(sticker: value.sticker, resource: nil)
    }

    func dismissMutationError() { mutationError = nil }

    private func reference(for route: GroupRoute) -> EntityReference? {
        guard name(for: route) != nil else { return nil }
        switch route.filter {
        case .collection(let id), .tag(let id): return EntityReference(id: id, generation: route.generation)
        default: return nil
        }
    }

    private func mutate(_ action: @escaping @MainActor () async throws -> Void) async -> Bool {
        guard !closed, !isMutating else { return false }
        isMutating = true
        mutationError = nil
        let operation = Task { @MainActor in
            var success = false
            do {
                try await action()
                success = true
            } catch {
                if !self.closed { self.mutationError = BridgeErrors.managementDescription(error) }
            }
            self.isMutating = false
            if !self.closed { await self.refresh() }
            return success
        }
        mutation = operation
        return await operation.value
    }

    func close() async {
        closed = true
        _ = await mutation?.value
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
