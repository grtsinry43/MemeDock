import Combine
import Foundation
import MemeDockBridge

/// Collections for the sidebar: names and ordering from `collections`, plus
/// counts and covers from `collection_summaries`.
@MainActor
final class CollectionCatalog: ObservableObject {
    @Published private(set) var active: [CollectionMetadata] = []
    @Published private(set) var trashed: [CollectionMetadata] = []
    @Published private(set) var summaries: [String: CollectionSummary] = [:]
    @Published private(set) var failure: String?

    private let session: LibrarySession
    private var reloading = false

    init(session: LibrarySession) {
        self.session = session
    }

    func load() async {
        guard !reloading else { return }
        reloading = true
        defer { reloading = false }
        do {
            let active = try await session.collections()
            let trashed = try await session.collections(deleted: true)
            let summaries = try await session.requireLibrary().collectionSummaries().awaitResult()
            self.active = active
            self.trashed = trashed
            self.summaries = Dictionary(
                summaries.map { ($0.collection.id, $0) },
                uniquingKeysWith: { first, _ in first }
            )
            self.failure = nil
        } catch {
            failure = describe(error)
        }
    }

    func summary(_ collectionId: String) -> CollectionSummary? {
        summaries[collectionId]
    }

    func create(name: String, before: String?) async throws {
        let created = try await session.createCollection(name: name, before: before)
        insertLocally(created, before: before)
    }

    func rename(_ collection: CollectionMetadata, name: String) async throws {
        let updated = try await session.renameCollection(reference(of: collection), name: name)
        if let index = active.firstIndex(where: { $0.id == updated.id }) {
            active[index] = updated
        }
    }

    func delete(_ collection: CollectionMetadata) async throws {
        _ = try await session.deleteCollection(reference(of: collection))
        await load()
    }

    func restore(_ collection: CollectionMetadata) async throws {
        _ = try await session.restoreCollection(
            reference(of: collection),
            deletedRevision: collection.lifecycle.revision
        )
        await load()
    }

    func move(_ collection: CollectionMetadata, before: String?) async throws {
        _ = try await session.moveCollection(reference(of: collection), before: before)
        await load()
    }

    func apply(_ event: SessionEvent) {
        switch event {
        case .collections, .reload, .sticker(_):
            Task { [weak self] in await self?.load() }
        case .tags, .thumbnail, .usage:
            break
        }
    }

    private func reference(of collection: CollectionMetadata) -> EntityReference {
        EntityReference(id: collection.id, generation: collection.lifecycle.generation)
    }

    /// Keeps the sidebar responsive: the new collection is placed immediately
    /// and the full reload that follows a notification replaces it.
    private func insertLocally(_ collection: CollectionMetadata, before: String?) {
        active.removeAll { $0.id == collection.id }
        if let before, let index = active.firstIndex(where: { $0.id == before }) {
            active.insert(collection, at: index)
        } else {
            active.append(collection)
        }
    }
}
