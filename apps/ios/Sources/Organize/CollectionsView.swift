import MemeDockBridge
import SwiftUI

struct CollectionsView: View {
    let model: OrganizeModel
    let thumbnails: ThumbnailStore
    @State private var reordering = false
    @State private var editing: GroupEditRequest?
    @State private var deleting: GroupRoute?

    init(model: OrganizeModel, thumbnails: ThumbnailStore, reordering: Bool = false) {
        self.model = model
        self.thumbnails = thumbnails
        _reordering = State(initialValue: reordering)
    }

    var body: some View {
        Group {
            if reordering {
                List {
                    ForEach(model.collections, id: \.collection.id) { summary in
                        NavigationLink(value: OrganizeDestination.group(route(summary.collection))) {
                            HStack {
                                Label(summary.collection.name, systemImage: "folder")
                                Spacer()
                                Text(summary.count.formatted()).foregroundStyle(.secondary)
                            }
                        }
                        .contextMenu {
                            Button("Rename", systemImage: "pencil") { editing = .rename(route(summary.collection)) }
                            Button("Move to Trash", systemImage: "trash", role: .destructive) {
                                deleting = route(summary.collection)
                            }
                        }
                    }
                    .onMove { indices, destination in
                        guard let index = indices.first, indices.count == 1, index < model.collections.count else {
                            return
                        }
                        let moved = model.collections[index].collection
                        var ordered = model.collections
                        ordered.move(fromOffsets: indices, toOffset: destination)
                        guard let position = ordered.firstIndex(where: { $0.collection.id == moved.id }) else { return }
                        let before = ordered.indices.contains(position + 1) ? ordered[position + 1].collection.id : nil
                        Task { _ = await model.moveCollection(route(moved), before: before) }
                    }
                    .moveDisabled(model.isMutating)
                }
            } else {
                ScrollView {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 152), spacing: 12)], spacing: 12) {
                        ForEach(model.collections, id: \.collection.id) { summary in
                            NavigationLink(value: OrganizeDestination.group(route(summary.collection))) {
                                CollectionCardContent(summary: summary, model: model, thumbnails: thumbnails)
                            }.buttonStyle(.plain)
                                .contextMenu {
                                    Button("Rename", systemImage: "pencil") {
                                        editing = .rename(route(summary.collection))
                                    }
                                    Button("Reorder Collections") { reordering = true }
                                    Button("Move to Trash", role: .destructive) { deleting = route(summary.collection) }
                                }
                        }
                    }.padding(16)
                }.background(Color(uiColor: .systemGroupedBackground))
            }
        }
        .navigationTitle("Collections")
        .toolbar(.hidden, for: .tabBar)
        .environment(\.editMode, .constant(reordering ? .active : .inactive))
        .overlay { if model.collections.isEmpty { ContentUnavailableView("No Collections", systemImage: "folder") } }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(reordering ? "Done" : "Reorder") { reordering.toggle() }.disabled(model.isMutating)
            }
            ToolbarItem(placement: .primaryAction) {
                Button("New Collection", systemImage: "plus") { editing = .newCollection }.disabled(model.isMutating)
            }
        }
        .modifier(GroupManagementPresentation(model: model, editing: $editing, deleting: $deleting))
        .refreshable { await model.refresh() }
    }

    private func route(_ collection: CollectionMetadata) -> GroupRoute {
        GroupRoute(filter: .collection(collection.id), generation: collection.lifecycle.generation)
    }
}

struct CollectionItemsOrderView: View {
    let session: LibrarySession
    let route: GroupRoute
    @State private var browser: LibraryBrowser?

    var body: some View {
        List {
            if let browser, let model = session.organize {
                ForEach(browser.items) { item in
                    HStack(spacing: 12) {
                        if let thumbnails = session.thumbnails {
                            StickerThumbnailView(item: item, store: thumbnails, open: { _ in }).frame(width: 56, height: 56)
                        }
                        Text(item.sticker.title)
                    }
                }
                .onMove { indices, destination in
                    guard let index = indices.first, indices.count == 1, index < browser.items.count else { return }
                    let moved = browser.items[index].sticker
                    var ordered = browser.items
                    ordered.move(fromOffsets: indices, toOffset: destination)
                    guard let position = ordered.firstIndex(where: { $0.id == moved.id }) else { return }
                    let before = ordered.indices.contains(position + 1) ? ordered[position + 1].id : nil
                    browser.moveLocally(moved.id, before: before)
                    Task {
                        _ = await model.moveSticker(moved, in: route, before: before)
                        await browser.refresh()
                    }
                }
                .moveDisabled(model.isMutating || browser.hasMore || browser.isLoading || browser.isLoadingMore)
                if let error = browser.pageError ?? browser.failure {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") {
                        Task {
                            if browser.pageError != nil { await browser.loadMore() } else { await browser.refresh() }
                        }
                    }
                }
                if let error = model.mutationError { Text(error).foregroundStyle(.red) }
                if browser.isLoading || browser.isLoadingMore || model.isMutating { ProgressView() }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Reorder Stickers").navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
        .environment(\.editMode, .constant(.active))
        .task(id: session.organize?.name(for: route)) {
            if browser == nil {
                browser = session.makeGroupBrowser(for: route)
                await browser?.refresh()
            }
            while let browser, browser.hasMore, browser.pageError == nil, !Task.isCancelled {
                await browser.loadMore()
            }
        }
    }
}
