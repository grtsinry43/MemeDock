import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Owns the models the library window needs and keeps them in step with the
/// core's notification stream. One subscriber feeds every model, so the grid,
/// the sidebar, and the detail pane cannot disagree about what changed.
@MainActor
final class LibraryModel: ObservableObject {
    let session: LibrarySession
    let status: StatusCenter
    let actions: StickerActions
    let imports: ImportCoordinator
    let browser: StickerBrowser
    let collections: CollectionCatalog
    let tags: TagCatalog
    let thumbnails: ThumbnailStore
    let selection: StickerSelection
    let detail: StickerDetailModel

    /// While on, a click extends the batch selection instead of only moving the
    /// detail pane: choosing many and inspecting one are different jobs, and the
    /// right column shows only one of them at a time.
    @Published private(set) var selecting = false

    private var subscriptions: Set<AnyCancellable> = []
    private var started = false

    init(session: LibrarySession, status: StatusCenter, clipboard: ClipboardGuard) {
        self.session = session
        self.status = status
        actions = StickerActions(session: session, clipboard: clipboard, status: status)
        imports = ImportCoordinator(session: session, status: status)
        browser = StickerBrowser(session: session)
        collections = CollectionCatalog(session: session)
        tags = TagCatalog(session: session)
        thumbnails = ThumbnailStore(session: session)
        selection = StickerSelection(session: session, status: status)
        detail = StickerDetailModel(session: session)
    }

    func start() async {
        guard !started else { return }
        started = true
        subscribe()
        await collections.load()
        await tags.load()
        browser.startIfNeeded()
    }

    func shutdown() {
        started = false
        subscriptions.removeAll()
        thumbnails.clear()
        selection.clear()
        detail.clear()
        selecting = false
    }

    func select(_ filter: LibraryFilter) {
        browser.filter = filter
    }

    /// Importing while a collection is open means adding to that collection,
    /// which is the only reading of "add" that keeps what is on screen whole.
    var importCollectionId: String? {
        if case let .collection(id) = browser.filter { return id }
        return nil
    }

    /// A click moves the detail pane, or extends the selection while picking
    /// many. The modifier follows the usual macOS convention.
    func click(_ item: StickerItem) {
        if selecting || NSEvent.modifierFlags.contains(.command) {
            selection.toggle(item)
            // One picked sticker can still be the one being inspected; from the
            // second on the batch pane takes the column over.
            if selection.count > 1 {
                setSelecting(true)
            }
            return
        }
        selection.clear()
        Task { await detail.load(id: item.id) }
    }

    /// Turns the picking mode on or off. Leaving it drops the selection: one
    /// that is no longer shown anywhere would keep rows tinted with nothing
    /// left to act on.
    func setSelecting(_ on: Bool) {
        guard selecting != on else { return }
        selecting = on
        if !on {
            selection.clear()
        }
    }

    /// A select-all is a batch in the making, so it opens the picking mode even
    /// when it only covers a single loaded sticker.
    func selectAll() {
        guard !browser.items.isEmpty else { return }
        selection.select(browser.items)
        setSelecting(true)
    }

    /// Whether a select-all has anything to pick up right now. The batch calls
    /// ask the same of the pane's own button, and a call already in flight would
    /// only be handed a second batch to carry.
    var canSelectAll: Bool {
        !selection.running && !browser.items.isEmpty
    }

    /// The sticker 拷贝 acts on: the one the detail column has open, which is the
    /// only row the library treats as focused. A trashed sticker is on its way
    /// out, and while a batch is being picked the right column belongs to the
    /// selection rather than to one sticker, so neither leaves anything to copy.
    var copyTarget: StickerItem? {
        guard !selecting, browser.filter != .deleted, !detail.loading,
              !actions.busy, detail.sticker?.originalPath != nil else { return nil }
        return detail.item
    }

    /// Copies `copyTarget`. The row in the grid is what a click opens, so this is
    /// the one-sticker copy the detail column already offers by button.
    func copyFocused() async {
        guard let item = copyTarget else { return }
        await perform(status) { try await self.actions.copyImage(item) }
    }

    /// Grid operations act on a row rather than the opened detail, so they go
    /// through the item the grid already holds.
    func setStarred(_ item: StickerItem, _ starred: Bool) async {
        await perform(status) {
            _ = try await self.session.patch(
                item.reference,
                edit: StickerEdit(title: nil, note: nil, starred: starred)
            )
        }
    }

    func delete(_ item: StickerItem) async {
        await perform(status) { _ = try await self.session.deleteSticker(item.reference) }
    }

    func restore(_ item: StickerItem) async {
        await perform(status) {
            _ = try await self.session.restoreSticker(
                item.reference,
                deletedRevision: item.sticker.lifecycle.revision
            )
        }
    }

    func clearSelection() {
        selection.clear()
    }

    func apply(_ event: SessionEvent) {
        browser.apply(event)
        collections.apply(event)
        tags.apply(event)
        detail.apply(event)
        switch event {
        case let .thumbnail(id):
            // The row keeps its place, so the store is asked directly; a changed
            // path is picked up by the store itself.
            thumbnails.invalidate(id)
            if let item = browser.items.first(where: { $0.id == id }) {
                thumbnails.load(item)
            }
        case .reload:
            thumbnails.clear()
        case .collections, .tags:
            // Deleting what is being browsed has to move the grid somewhere, and
            // the trashed entry keeps the section it used to be only as a row.
            guard !filterStillExists() else { return }
            browser.filter = .all
        case .sticker, .usage:
            break
        }
    }

    private func filterStillExists() -> Bool {
        switch browser.filter {
        case let .collection(id): return collections.active.contains { $0.id == id }
        case let .tag(id): return tags.active.contains { $0.id == id }
        case .all, .starred, .recentlyUsed, .deleted: return true
        }
    }

    private func subscribe() {
        session.events
            .sink { [weak self] event in
                Task { @MainActor in self?.apply(event) }
            }
            .store(in: &subscriptions)

        // A section change replaces every row, so a selection or an open detail
        // pane would otherwise point at something no longer on screen.
        browser.$filter
            .removeDuplicates()
            .dropFirst()
            .sink { [weak self] _ in
                Task { @MainActor in self?.sectionChanged() }
            }
            .store(in: &subscriptions)

        // Asking for thumbnails from the grid contents covers the rows that
        // arrive later and the reloads that emptied the store; the store itself
        // skips requests it already serves.
        browser.$items
            .sink { [weak self] items in
                Task { @MainActor in
                    guard let self else { return }
                    for item in items {
                        self.thumbnails.load(item)
                    }
                }
            }
            .store(in: &subscriptions)
    }

    private func sectionChanged() {
        selection.clear()
        detail.clear()
        // Collection order only means something inside a collection, so the
        // picker must not keep showing a choice the new section cannot honour.
        if !browser.filter.allowsCollectionOrder, browser.sort == .collectionOrder {
            browser.sort = .recent
        }
    }
}
