import Foundation
import MemeDockBridge
import Observation

@MainActor
@Observable
final class LibraryBrowser {
    var searchText = "" {
        didSet { if searchText != oldValue, !changingQuery { restart(debounce: true) } }
    }
    var filter: LibraryFilter = .recentlyUsed {
        didSet { if filter != oldValue, !changingQuery { restart() } }
    }
    private(set) var items: [LibraryItem] = []
    private(set) var collections: [CollectionMetadata] = []
    private(set) var collectionError: String?
    private(set) var failure: String?
    private(set) var pageError: String?
    private(set) var isLoading = false
    private(set) var isLoadingMore = false
    private(set) var hasMore = false
    private(set) var scrollTarget: String?
    private(set) var isLocating = false
    var visibleStickerID: String?
    private(set) var isSelecting = false
    var isPresentingActions = false
    private(set) var selected: [String: StickerMetadata] = [:]
    private(set) var isMutating = false
    private(set) var actionError: String?
    private(set) var actionCompleted = 0
    private(set) var actionTotal = 0
    @ObservationIgnored private var batchTask: BatchTask?

    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let pageSize: UInt32
    @ObservationIgnored private var cursor: QueryCursorHandle?
    @ObservationIgnored private var pageTask: StickerPageTask?
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var changeWork: Task<Void, Never>?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var catalogGeneration: UInt64 = 0
    @ObservationIgnored private var changingQuery = false
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var locatingTarget: String?
    @ObservationIgnored private let scope: LibraryFilter?

    init(library: LibraryHandle, pageSize: UInt32 = 60, scope: LibraryFilter? = nil) {
        self.library = library
        self.pageSize = min(200, max(1, pageSize))
        self.scope = scope
        if let scope { filter = scope }
    }

    deinit {
        try? batchTask?.cancel()
        work?.cancel()
        changeWork?.cancel()
        _ = pageTask?.cancel()
    }

    var filterTitle: String {
        switch filter {
        case .recentlyUsed: String(localized: "Recently Used")
        case .all: String(localized: "All Stickers")
        case .starred: String(localized: "Favorites")
        case .collection(let id): collections.first { $0.id == id }?.name ?? String(localized: "Collection")
        case .tag: String(localized: "Tag")
        }
    }

    func refresh() async {
        changeWork?.cancel()
        beginLoad(preservingItems: true, target: locatingTarget)
        await work?.value
    }

    func waitUntilSettled() async {
        await changeWork?.value
        await work?.value
    }

    func loadMore() async {
        guard hasMore, !isLoading, !isLoadingMore, !isLocating, !closed else { return }
        beginLoad(more: true)
        await work?.value
    }

    /// Business notifications are coalesced; thumbnail completion never restarts pagination.
    func libraryChanged() {
        guard !closed else { return }
        // Invalidate immediately so an outstanding page cannot publish stale membership.
        invalidate()
        hasMore = false
        isLoadingMore = false
        changeWork?.cancel()
        changeWork = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
            guard let self, !self.closed, !Task.isCancelled else { return }
            self.beginLoad(preservingItems: true, target: self.locatingTarget)
        }
    }

    func loadCollections() async {
        guard !closed else { return }
        catalogGeneration &+= 1
        let current = catalogGeneration
        do {
            let values = try await library.collections(deleted: false).awaitResult()
            guard !closed, catalogGeneration == current else { return }
            collections = values
            collectionError = nil
            if scope == nil, case .collection(let id) = filter, !values.contains(where: { $0.id == id }) {
                filter = .all
            }
        } catch {
            guard !closed, catalogGeneration == current else { return }
            collectionError = BridgeErrors.browseDescription(error)
        }
    }

    func refreshThumbnail(_ id: String) async {
        guard !closed, items.contains(where: { $0.id == id }) else { return }
        let current = generation
        do {
            let resources = try await library.stickerResources(ids: [id]).awaitResult()
            guard !closed, generation == current,
                let resource = resources.first(where: { $0.asset.hash == id }),
                let index = items.firstIndex(where: { $0.id == id })
            else { return }
            items[index].resource = resource
            if resource.thumbnailStatus == .ready || resource.thumbnailStatus == .failed {
                items[index].thumbnailRevision &+= 1
            }
        } catch {
            // A thumbnail refresh failure leaves the grid intact; the tile can retry generation.
        }
    }

    func invalidateThumbnails() {
        for index in items.indices { items[index].thumbnailRevision &+= 1 }
    }

    func showImported(_ id: String) async {
        changingQuery = true
        searchText = ""
        filter = .all
        changingQuery = false
        scrollTarget = nil
        changeWork?.cancel()
        beginLoad(target: id)
        await work?.value
    }

    func stop() {
        closed = true
        try? batchTask?.cancel()
        selected = [:]
        isSelecting = false
        catalogGeneration &+= 1
        changeWork?.cancel()
        invalidate()
        items = []
        collections = []
        cursor = nil
        hasMore = false
        isLoading = false
        isLoadingMore = false
        isLocating = false
        locatingTarget = nil
    }

    private func restart(debounce: Bool = false) {
        if !isMutating { endSelection() }
        changeWork?.cancel()
        beginLoad(debounce: debounce)
    }

    func startSelection() { isSelecting = true }

    func endSelection() {
        guard !isMutating else { return }
        isSelecting = false
        selected = [:]
    }

    func toggleSelection(_ item: LibraryItem) {
        guard !isMutating else { return }
        if selected.removeValue(forKey: item.id) == nil {
            guard selected.count < 1000 else {
                actionError = String(localized: "Select up to 1,000 stickers at a time.")
                return
            }
            selected[item.id] = item.sticker
        }
    }

    func dismissActionError() { actionError = nil }
    func moveLocally(_ id: String, before: String?) {
        guard !closed, !isLoading, !hasMore, let index = items.firstIndex(where: { $0.id == id }), id != before else {
            return
        }
        let item = items.remove(at: index)
        let destination = before.flatMap { next in items.firstIndex { $0.id == next } } ?? items.endIndex
        items.insert(item, at: destination)
    }
    func cancelAction() { try? batchTask?.cancel() }

    func apply(_ action: BatchAction, to stickers: [StickerMetadata]? = nil) async -> Bool {
        guard !closed, !isMutating else { return false }
        let targets = stickers ?? Array(selected.values)
        guard !targets.isEmpty else { return false }
        isMutating = true
        actionError = nil
        actionCompleted = 0
        actionTotal = targets.count
        defer {
            isMutating = false
            batchTask = nil
        }
        do {
            let task = try library.batch(
                targets: targets.map {
                    BatchTarget(
                        target: EntityReference(id: $0.id, generation: $0.lifecycle.generation), deletedRevision: nil)
                }, action: action)
            batchTask = task
            let polling = Task { [weak browser = self] in
                while !Task.isCancelled {
                    browser?.actionCompleted = task.snapshot().items.filter { $0.outcome != .pending }.count
                    do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
                }
            }
            let report: BatchReport
            do { report = try await task.awaitResult() } catch {
                polling.cancel()
                await polling.value
                throw error
            }
            polling.cancel()
            await polling.value
            actionCompleted = report.items.filter { $0.outcome != .pending }.count
            guard !closed else { return false }
            let failed = report.items.filter { $0.outcome == .failed || $0.outcome == .pending }
            if !failed.isEmpty || report.stopped {
                let failures = failed.map { result in
                    let title = targets.first { $0.id == result.id }?.title ?? result.id
                    let message =
                        result.error.map {
                            BridgeErrors.managementDescription(BridgeError.Failure(code: $0, detail: ""))
                        }
                        ?? String(localized: "Not completed")
                    return "\(title): \(message)"
                }
                actionError =
                    ([report.stopped ? String(localized: "The operation stopped. Completed changes are kept.") : nil]
                    .compactMap { $0 } + failures).joined(separator: "\n")
                let unresolved = Set(failed.map(\.id))
                selected = selected.filter { unresolved.contains($0.key) }
            } else {
                selected = [:]
            }
            await refresh()
            return failed.isEmpty && !report.stopped
        } catch {
            guard !closed else { return false }
            actionError = BridgeErrors.managementDescription(error)
            return false
        }
    }

    private func invalidate() {
        generation &+= 1
        _ = pageTask?.cancel()
        pageTask = nil
        work?.cancel()
    }

    private func query(cursor: QueryCursorHandle?) -> StickerQuery {
        let filter = scope ?? self.filter
        let collection: String?
        if case .collection(let id) = filter { collection = id } else { collection = nil }
        let tags: [String]
        if case .tag(let id) = filter { tags = [id] } else { tags = [] }
        return StickerQuery(
            requestId: newRequestId(), text: searchText.trimmingCharacters(in: .whitespacesAndNewlines),
            collectionId: collection, tagIds: tags, starred: filter == .starred ? true : nil,
            deleted: false, sort: filter.sort, pageSize: pageSize, cursor: cursor
        )
    }

    private func beginLoad(
        more: Bool = false, preservingItems: Bool = false,
        debounce: Bool = false, target: String? = nil
    ) {
        guard !closed else { return }
        let oldCount = items.count
        let nextCursor = more ? cursor : nil
        invalidate()
        let current = generation
        var request = query(cursor: nextCursor)
        if !more {
            cursor = nil
            hasMore = false
            if !preservingItems { items = [] }
        }
        failure = nil
        pageError = nil
        isLoading = !more
        isLoadingMore = more
        isLocating = target != nil
        locatingTarget = target
        work = Task { [weak self] in
            guard let self else { return }
            defer {
                if generation == current {
                    isLoading = false
                    isLoadingMore = false
                    isLocating = false
                    pageTask = nil
                }
            }
            do {
                if debounce { try await Task.sleep(for: .milliseconds(250)) }
                var loaded: [LibraryItem] = []
                var loadedIDs: Set<String> = []
                var next: QueryCursorHandle?
                repeat {
                    try Task.checkCancellation()
                    let task = try library.listStickers(query: request)
                    pageTask = task
                    let page = try await task.awaitResult()
                    guard !closed, generation == current, page.requestId == request.requestId else { return }
                    let resources = Dictionary(
                        page.resources.map { ($0.asset.hash, $0) }, uniquingKeysWith: { first, _ in first })
                    loaded += page.stickers.filter { loadedIDs.insert($0.id).inserted }.map {
                        LibraryItem(sticker: $0, resource: resources[$0.id])
                    }
                    next = page.next
                    request.cursor = next
                    request.requestId = newRequestId()
                    if let target, loaded.contains(where: { $0.id == target }) { break }
                    if target == nil, loaded.count >= (preservingItems ? max(oldCount, Int(pageSize)) : Int(pageSize)) {
                        break
                    }
                } while next != nil
                guard !closed, generation == current else { return }
                if more {
                    let known = Set(items.map(\.id))
                    items += loaded.filter { !known.contains($0.id) }
                } else {
                    // Retain thumbnail revisions across an in-place business refresh.
                    let revisions = Dictionary(
                        items.map { ($0.id, $0.thumbnailRevision) }, uniquingKeysWith: { first, _ in first })
                    items = loaded.map { value in
                        var value = value
                        value.thumbnailRevision = revisions[value.id] ?? 0
                        return value
                    }
                }
                cursor = next
                hasMore = next != nil
                if let target {
                    if items.contains(where: { $0.id == target }) {
                        scrollTarget = target
                    } else {
                        failure = String(localized: "This imported sticker is no longer available in the library.")
                    }
                    locatingTarget = nil
                }
            } catch {
                guard !closed, generation == current, !Task.isCancelled else { return }
                if more {
                    pageError = BridgeErrors.browseDescription(error)
                } else {
                    failure = BridgeErrors.browseDescription(error)
                }
            }
        }
    }
}
