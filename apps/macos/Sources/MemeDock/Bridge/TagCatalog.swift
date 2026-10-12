import Combine
import Foundation
import MemeDockBridge

/// Tags for the sidebar. The core hands them out in normalized-name order and
/// offers no reordering, so a locally created tag is inserted at that same
/// position until the reload triggered by its notification replaces the list.
@MainActor
final class TagCatalog: ObservableObject {
    @Published private(set) var active: [TagMetadata] = []
    @Published private(set) var trashed: [TagMetadata] = []
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
            let active = try await session.tags()
            let trashed = try await session.tags(deleted: true)
            self.active = active
            self.trashed = trashed
            self.failure = nil
        } catch {
            failure = describe(error)
        }
    }

    func create(name: String) async throws {
        insertLocally(try await session.createTag(name: name))
    }

    func rename(_ tag: TagMetadata, name: String) async throws {
        let updated = try await session.renameTag(reference(of: tag), name: name)
        active.removeAll { $0.id == updated.id }
        insertLocally(updated)
    }

    func delete(_ tag: TagMetadata) async throws {
        _ = try await session.deleteTag(reference(of: tag))
        await load()
    }

    func restore(_ tag: TagMetadata) async throws {
        _ = try await session.restoreTag(reference(of: tag), deletedRevision: tag.lifecycle.revision)
        await load()
    }

    func apply(_ event: SessionEvent) {
        switch event {
        case .tags, .reload:
            Task { [weak self] in await self?.load() }
        case .collections, .sticker, .thumbnail, .usage:
            break
        }
    }

    private func reference(of tag: TagMetadata) -> EntityReference {
        EntityReference(id: tag.id, generation: tag.lifecycle.generation)
    }

    private func insertLocally(_ tag: TagMetadata) {
        active.removeAll { $0.id == tag.id }
        let index = active.firstIndex { $0.normalizedName > tag.normalizedName } ?? active.endIndex
        active.insert(tag, at: index)
    }
}
