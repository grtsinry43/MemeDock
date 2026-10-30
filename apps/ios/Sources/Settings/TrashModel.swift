import Foundation
import MemeDockBridge
import Observation

struct TrashRow: Identifiable {
    let target: TrashTarget
    let title: String
    var item: LibraryItem? = nil
    var id: String { "\(target.kind)-\(target.target.id)-\(target.target.generation)" }
}

@MainActor
@Observable
final class TrashModel {
    private(set) var rows: [TrashRow] = []
    private(set) var isLoading = false
    private(set) var isMutating = false
    private(set) var completed = 0
    private(set) var total = 0
    private(set) var error: String?
    private(set) var hasMore = false
    private(set) var allTargets: [TrashTarget] = []
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private var cursor: QueryCursorHandle?
    @ObservationIgnored private var reader: Task<Void, Never>?
    @ObservationIgnored private var mutation: Task<Void, Never>?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var stopRequested = false

    init(library: LibraryHandle) { self.library = library }

    func load(more: Bool = false) async {
        guard !closed, !isMutating, !more || (hasMore && !isLoading) else { return }
        generation &+= 1
        let current = generation
        reader?.cancel()
        isLoading = true
        let next = more ? cursor : nil
        reader = Task {
            defer { if self.generation == current { self.isLoading = false } }
            do {
                let snapshot = more ? self.allTargets : try await self.library.trashSnapshot().awaitResult()
                let collections = more ? [] : try await self.library.collections(deleted: true).awaitResult()
                let tags = more ? [] : try await self.library.tags(deleted: true).awaitResult()
                let query = StickerQuery(
                    requestId: newRequestId(), text: "", collectionId: nil, tagIds: [],
                    starred: nil, deleted: true, sort: .recent, pageSize: 60, cursor: next)
                let page = try await self.library.listStickers(query: query).awaitResult()
                guard !self.closed, self.generation == current, !Task.isCancelled else { return }
                let targets = Dictionary(
                    snapshot.map { ("\($0.kind)-\($0.target.id)", $0) }, uniquingKeysWith: { first, _ in first })
                var values: [TrashRow] = []
                for value in collections {
                    if let target = targets["\(TrashEntityKind.collection)-\(value.id)"] {
                        values.append(TrashRow(target: target, title: value.name))
                    }
                }
                for value in tags {
                    if let target = targets["\(TrashEntityKind.tag)-\(value.id)"] {
                        values.append(TrashRow(target: target, title: value.name))
                    }
                }
                for value in page.stickers {
                    if let target = targets["\(TrashEntityKind.sticker)-\(value.id)"] {
                        let resource = page.resources.first { $0.asset.hash == value.id }
                        values.append(
                            TrashRow(
                                target: target, title: value.title,
                                item: LibraryItem(sticker: value, resource: resource)))
                    }
                }
                if more {
                    let existing = Set(self.rows.map(\.id))
                    self.rows += values.filter { !existing.contains($0.id) }
                } else {
                    self.rows = values
                    self.allTargets = snapshot
                }
                self.cursor = page.next
                self.hasMore = page.next != nil
                self.error = nil
            } catch {
                guard !self.closed, self.generation == current, !Task.isCancelled else { return }
                self.error = BridgeErrors.managementDescription(error)
            }
        }
        await reader?.value
    }

    func stop() { stopRequested = true }

    func retryCleanup() async {
        guard !closed, !isMutating else { return }
        isMutating = true
        mutation = Task {
            do {
                _ = try await library.retryPurgeCleanup().awaitResult()
                error = nil
            } catch { self.error = BridgeErrors.managementDescription(error) }
            isMutating = false
        }
        await mutation?.value
    }

    func apply(to targets: [TrashTarget], permanently: Bool) async {
        guard !closed, !isMutating, !targets.isEmpty else { return }
        generation &+= 1
        reader?.cancel()
        isLoading = false
        isMutating = true
        stopRequested = false
        completed = 0
        total = targets.count
        error = nil
        mutation = Task {
            var failed = 0
            var message: String?
            for target in targets {
                if self.stopRequested || self.closed { break }
                do {
                    if permanently {
                        switch target.kind {
                        case .sticker:
                            _ = try await self.library.purgeSticker(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        case .collection:
                            _ = try await self.library.purgeCollection(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        case .tag:
                            _ = try await self.library.purgeTag(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        }
                    } else {
                        switch target.kind {
                        case .sticker:
                            _ = try await self.library.restoreSticker(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        case .collection:
                            _ = try await self.library.restoreCollection(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        case .tag:
                            _ = try await self.library.restoreTag(
                                target: target.target, deletedRevision: target.deletedRevision
                            ).awaitResult()
                        }
                    }
                    self.completed += 1
                } catch {
                    failed += 1
                    message = BridgeErrors.managementDescription(error)
                }
            }
            let stopped = self.stopRequested
            self.isMutating = false
            if !self.closed {
                await self.load()
                if failed > 0 {
                    self.error = message
                } else if stopped {
                    self.error = String(localized: "The operation stopped. Completed changes are kept.")
                }
            }
        }
        await mutation?.value
    }

    func close() async {
        closed = true
        generation &+= 1
        reader?.cancel()
        stopRequested = true
        await mutation?.value
        await reader?.value
        rows = []
        allTargets = []
    }
}
