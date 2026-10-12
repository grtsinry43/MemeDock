import Combine
import Foundation
import MemeDockBridge

enum LibraryFilter: Equatable, Hashable {
    case all
    case starred
    case recentlyUsed
    case deleted
    case collection(String)
    case tag(String)

    /// Collection order only means something inside a collection, so every
    /// other section falls back to the most recent first.
    var allowsCollectionOrder: Bool {
        if case .collection = self { return true }
        return false
    }
}

/// Paging query state for the grid. Searches are debounced, and responses from
/// a superseded request are dropped instead of overwriting newer results.
@MainActor
final class StickerBrowser: ObservableObject {
    private static let pageSize: UInt32 = 80

    @Published var filter: LibraryFilter = .all {
        didSet { if filter != oldValue { restart() } }
    }
    @Published var sort: StickerSort = .recent {
        didSet { if sort != oldValue { restart() } }
    }
    @Published var searchText: String = "" {
        didSet { if searchText != oldValue { scheduleSearch() } }
    }
    @Published private(set) var items: [StickerItem] = []
    @Published private(set) var isLoading = false
    @Published private(set) var isLoadingMore = false
    @Published private(set) var hasMore = false
    @Published private(set) var failure: String?

    private let session: LibrarySession
    private var cursor: QueryCursorHandle?
    private var activeRequest: String?
    private var pageTask: StickerPageTask?
    private var pageWork: Task<Void, Never>?
    private var searchWork: Task<Void, Never>?

    init(session: LibrarySession) {
        self.session = session
    }

    var effectiveSort: StickerSort {
        guard filter.allowsCollectionOrder else {
            return sort == .collectionOrder ? .recent : sort
        }
        return sort
    }

    func startIfNeeded() {
        guard items.isEmpty, !isLoading, failure == nil else { return }
        reload()
    }

    func reload() {
        cursor = nil
        hasMore = false
        load(more: false)
    }

    func restart() {
        cursor = nil
        items = []
        hasMore = false
        load(more: false)
    }

    func loadMore() {
        guard hasMore, !isLoading, !isLoadingMore else { return }
        load(more: true)
    }

    /// Applies a library change to the items already on screen.
    func apply(_ event: SessionEvent) {
        switch event {
        case .reload:
            restart()
        case .collections, .tags:
            break
        case let .sticker(id):
            refresh(id, reloadRow: false)
        case let .thumbnail(id):
            refreshResource(id)
        case let .usage(id):
            if effectiveSort == .lastUsed { refresh(id, reloadRow: false) }
        }
    }

    private func scheduleSearch() {
        searchWork?.cancel()
        searchWork = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 300_000_000)
            guard !Task.isCancelled, let self else { return }
            self.restart()
        }
    }

    private func load(more: Bool) {
        guard let library = session.library else { return }
        // Same as import: the previous page's outcome arrives on its own
        // progress stream, so the unified cancel reply carries nothing extra.
        _ = pageTask?.cancel()
        if !more {
            isLoading = true
            isLoadingMore = false
        } else {
            isLoadingMore = true
        }
        failure = nil
        let requestId = newRequestId()
        activeRequest = requestId
        let query = StickerQuery(
            requestId: requestId,
            text: searchText.trimmingCharacters(in: .whitespacesAndNewlines),
            collectionId: collectionId,
            tagIds: tagIds,
            starred: filter == .starred ? true : nil,
            deleted: filter == .deleted,
            sort: effectiveSort,
            pageSize: Self.pageSize,
            cursor: more ? cursor : nil
        )
        pageWork?.cancel()
        pageWork = Task { [weak self] in
            do {
                let task = try library.listStickers(query: query)
                self?.pageTask = task
                let page = try await task.awaitResult()
                guard let self, self.activeRequest == requestId else { return }
                self.apply(page: page, more: more)
            } catch {
                guard let self, self.activeRequest == requestId else { return }
                if self.isLoading || self.isLoadingMore {
                    self.failure = describe(error)
                }
            }
            guard let self, self.activeRequest == requestId else { return }
            self.isLoading = false
            self.isLoadingMore = false
            self.pageTask = nil
        }
    }

    private func apply(page: StickerPage, more: Bool) {
        // The page carries stickers and resources as two parallel arrays, and
        // the core does not promise that resources come back in sticker order,
        // so pair them by content hash (the sticker id) instead of by position.
        let resources = Self.byHash(page.resources)
        let added = page.stickers.map { sticker in
            StickerItem(sticker: sticker, resource: resources[sticker.id])
        }
        items = more ? items + added : added
        cursor = page.next
        hasMore = page.next != nil
        isLoading = false
        isLoadingMore = false
        let missing = added.filter { $0.asset == nil }.map(\.id)
        if !missing.isEmpty {
            Task { [weak self] in
                guard let self, let resources = try? await self.session.resources(missing) else { return }
                let fetched = Self.byHash(resources)
                for stickerId in missing {
                    guard let resource = fetched[stickerId],
                          let index = self.items.firstIndex(where: { $0.id == stickerId })
                    else { continue }
                    self.items[index].apply(resource: resource)
                }
            }
        }
    }

    /// Resources are addressed by content hash, which is also the sticker id;
    /// the hash is the asset primary key, so folding is only a safety net.
    private static func byHash(_ resources: [StickerResource]) -> [String: StickerResource] {
        let pairs = resources.map { ($0.asset.hash, $0) }
        return Dictionary(pairs, uniquingKeysWith: { first, _ in first })
    }

    private var collectionId: String? {
        if case let .collection(id) = filter { return id }
        return nil
    }

    private var tagIds: [String] {
        if case let .tag(id) = filter { return [id] }
        return []
    }

    private func refresh(_ stickerId: String, reloadRow: Bool) {
        Task { [weak self] in
            guard let self else { return }
            do {
                let detail = try await self.session.detail(stickerId)
                guard let index = self.items.firstIndex(where: { $0.id == stickerId }) else {
                    if self.matches(detail) { self.restart() }
                    return
                }
                if self.matches(detail) {
                    self.items[index].apply(detail: detail)
                } else {
                    self.items.remove(at: index)
                }
            } catch {
                self.items.removeAll { $0.id == stickerId }
            }
        }
    }

    private func refreshResource(_ stickerId: String) {
        Task { [weak self] in
            guard let self else { return }
            guard let resources = try? await self.session.resources([stickerId]),
                  let resource = resources.first,
                  let index = self.items.firstIndex(where: { $0.id == stickerId })
            else { return }
            self.items[index].apply(resource: resource)
        }
    }

    private func matches(_ detail: StickerDetail) -> Bool {
        let deleted = detail.sticker.lifecycle.deletedAt != nil
        switch filter {
        case .deleted:
            return deleted
        case .all, .recentlyUsed:
            return !deleted
        case .starred:
            return !deleted && detail.sticker.starred
        case let .collection(id):
            return !deleted && detail.collection?.id == id
        case let .tag(id):
            return !deleted && detail.tags.contains { $0.id == id }
        }
    }
}
