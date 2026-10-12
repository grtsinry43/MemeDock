import Foundation
import MemeDockBridge

/// Thin async wrappers around the core mutations. Views stay free of task
/// handles: each call runs the operation to completion and reports failures.
extension LibrarySession {
    func detail(_ stickerId: String) async throws -> StickerDetail {
        try await requireLibrary().stickerDetail(id: stickerId).awaitResult()
    }

    func resources(_ stickerIds: [String]) async throws -> [StickerResource] {
        try await requireLibrary().stickerResources(ids: stickerIds).awaitResult()
    }

    func collections(deleted: Bool = false) async throws -> [CollectionMetadata] {
        try await requireLibrary().collections(deleted: deleted).awaitResult()
    }

    func tags(deleted: Bool = false) async throws -> [TagMetadata] {
        try await requireLibrary().tags(deleted: deleted).awaitResult()
    }

    func requestThumbnail(_ stickerId: String, priority: Priority) async throws -> String {
        let thumbnail = try await requireLibrary()
            .requestThumbnail(id: stickerId, priority: priority)
            .awaitResult()
        return thumbnail.path
    }

    func patch(_ reference: EntityReference, edit: StickerEdit) async throws -> StickerMetadata {
        try await requireLibrary().patchSticker(target: reference, edit: edit).awaitResult()
    }

    func deleteSticker(_ reference: EntityReference) async throws -> StickerMetadata {
        try await requireLibrary().deleteSticker(target: reference).awaitResult()
    }

    func restoreSticker(_ reference: EntityReference, deletedRevision: Int64) async throws -> StickerMetadata {
        try await requireLibrary()
            .restoreSticker(target: reference, deletedRevision: deletedRevision)
            .awaitResult()
    }

    /// How a call treats the sticker's collection.
    enum CollectionAssignment {
        /// Leave the current collection alone.
        case unchanged
        /// Take the sticker out of its collection.
        case cleared
        case assign(EntityReference)
    }

    /// Passing `nil` for `tags` keeps the current tag set while an empty array
    /// clears it. The collection needs an explicit instruction because leaving
    /// it alone and clearing it are different requests.
    @discardableResult
    func setOrganization(
        sticker: EntityReference,
        collection: CollectionAssignment = .unchanged,
        tags: [EntityReference]? = nil
    ) async throws -> Bool {
        let change: Bool
        let reference: EntityReference?
        switch collection {
        case .unchanged:
            change = false
            reference = nil
        case .cleared:
            change = true
            reference = nil
        case let .assign(target):
            change = true
            reference = target
        }
        return try await requireLibrary().setStickerOrganization(
            target: sticker,
            changeCollection: change,
            collection: reference,
            tags: tags
        ).awaitResult()
    }

    func createCollection(name: String, before: String?) async throws -> CollectionMetadata {
        try await requireLibrary().createCollection(name: name, before: before).awaitResult()
    }

    func renameCollection(_ reference: EntityReference, name: String) async throws -> CollectionMetadata {
        try await requireLibrary().renameCollection(target: reference, name: name).awaitResult()
    }

    func deleteCollection(_ reference: EntityReference) async throws -> CollectionMetadata {
        try await requireLibrary().deleteCollection(target: reference).awaitResult()
    }

    func moveCollection(_ reference: EntityReference, before: String?) async throws -> CollectionMetadata {
        try await requireLibrary().moveCollection(target: reference, before: before).awaitResult()
    }

    func createTag(name: String) async throws -> TagMetadata {
        try await requireLibrary().createTag(name: name).awaitResult()
    }

    @discardableResult
    func record(_ stickerId: String, action: UsageAction) async throws -> UsageMetadata {
        try await requireLibrary().recordUse(id: stickerId, action: action).awaitResult()
    }

    func renameTag(_ reference: EntityReference, name: String) async throws -> TagMetadata {
        try await requireLibrary().renameTag(target: reference, name: name).awaitResult()
    }

    func deleteTag(_ reference: EntityReference) async throws -> TagMetadata {
        try await requireLibrary().deleteTag(target: reference).awaitResult()
    }

    /// A restore has to name the revision the entity had while deleted, which is
    /// the revision it still carries in the trash.
    func restoreTag(_ reference: EntityReference, deletedRevision: Int64) async throws -> TagMetadata {
        try await requireLibrary()
            .restoreTag(target: reference, deletedRevision: deletedRevision)
            .awaitResult()
    }

    func restoreCollection(_ reference: EntityReference, deletedRevision: Int64) async throws -> CollectionMetadata {
        try await requireLibrary()
            .restoreCollection(target: reference, deletedRevision: deletedRevision)
            .awaitResult()
    }

    func restoreSuggestions(_ stickerId: String) async throws -> RestoreSuggestions {
        try await requireLibrary().restoreSuggestions(id: stickerId).awaitResult()
    }

    func requireLibrary() throws -> LibraryHandle {
        guard let library else { throw AppFailure(.errorClosed) }
        return library
    }
}
