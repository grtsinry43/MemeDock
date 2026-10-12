import Combine
import Foundation
import MemeDockBridge

/// The sticker the detail column shows and the edits it offers. The core stays
/// authoritative: a mutation is reflected locally only until the notification
/// stream brings the stored value back.
@MainActor
final class StickerDetailModel: ObservableObject {
    @Published private(set) var sticker: StickerDetail?
    @Published private(set) var loading = false
    @Published private(set) var failure: String?

    /// What the sticker belonged to before it was deleted. A restore does not
    /// bring that back, so it is offered as a separate step.
    @Published private(set) var suggestions: RestoreSuggestions?

    private let session: LibrarySession
    private var pending: String?
    private var reference: EntityReference?

    init(session: LibrarySession) {
        self.session = session
    }

    var id: String? { sticker?.sticker.id }

    /// The grid cell shape the action helpers expect.
    var item: StickerItem? { sticker.map(StickerItem.init(detail:)) }

    func load(id: String) async {
        pending = id
        loading = true
        do {
            let loaded = try await session.detail(id)
            // A slower answer for an earlier selection must not win.
            guard pending == id else { return }
            sticker = loaded
            reference = EntityReference(id: loaded.sticker.id, generation: loaded.sticker.lifecycle.generation)
            suggestions = nil
            failure = nil
        } catch {
            guard pending == id else { return }
            sticker = nil
            reference = nil
            failure = describe(error)
        }
        if pending == id { loading = false }
    }

    func reload() async {
        guard let id else { return }
        await load(id: id)
    }

    func clear() {
        pending = nil
        loading = false
        sticker = nil
        reference = nil
        suggestions = nil
        failure = nil
    }

    func apply(_ event: SessionEvent) {
        guard case let .sticker(changed) = event, changed == id else { return }
        Task { [weak self] in await self?.reload() }
    }

    func patch(title: String? = nil, note: String? = nil, starred: Bool? = nil) async {
        guard let reference else { return }
        do {
            let updated = try await session.patch(
                reference,
                edit: StickerEdit(title: title, note: note, starred: starred)
            )
            if var current = sticker, current.sticker.id == updated.id {
                current.sticker = updated
                sticker = current
            }
        } catch {
            failure = describe(error)
        }
    }

    func assign(collection: LibrarySession.CollectionAssignment) async {
        guard let reference else { return }
        do {
            _ = try await session.setOrganization(sticker: reference, collection: collection)
        } catch {
            failure = describe(error)
        }
    }

    func assign(tags: [TagMetadata]) async {
        guard let reference else { return }
        let references = tags.map { EntityReference(id: $0.id, generation: $0.lifecycle.generation) }
        do {
            _ = try await session.setOrganization(sticker: reference, tags: references)
        } catch {
            failure = describe(error)
        }
    }

    func delete() async {
        guard let reference else { return }
        do {
            _ = try await session.deleteSticker(reference)
            clear()
        } catch {
            failure = describe(error)
        }
    }

    func restore() async {
        guard let current = sticker, let reference else { return }
        do {
            // Read the pre-deletion organisation while the sticker still exists,
            // then restore. The restore raises the generation, so the suggestion
            // is applied to whatever reference the reload reports afterwards.
            let hint = try? await session.restoreSuggestions(current.sticker.id)
            _ = try await session.restoreSticker(reference, deletedRevision: current.sticker.lifecycle.revision)
            suggestions = hint
            await reload()
        } catch {
            failure = describe(error)
        }
    }

    /// Puts the sticker back where it was before it was deleted.
    func applySuggestions() async {
        guard let reference, let suggestions else { return }
        let tags = suggestions.tags.map { EntityReference(id: $0.id, generation: $0.lifecycle.generation) }
        let collection: LibrarySession.CollectionAssignment
        if let target = suggestions.collection {
            collection = .assign(EntityReference(id: target.id, generation: target.lifecycle.generation))
        } else {
            collection = .cleared
        }
        do {
            _ = try await session.setOrganization(sticker: reference, collection: collection, tags: tags)
            self.suggestions = nil
            await reload()
        } catch {
            failure = describe(error)
        }
    }
}
