import SwiftUI

struct LibraryGridView: View {
    let browser: LibraryBrowser
    let thumbnails: ThumbnailStore
    let imports: ImportCoordinator?
    let sessionError: String?
    let refresh: () async -> Void
    let showProblems: () -> Void
    let importImages: () -> Void
    let transitionNamespace: Namespace.ID
    let open: (LibraryItem) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        @Bindable var browser = browser
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 16) {
                    if let imports, !imports.items.isEmpty {
                        ImportStatusView(coordinator: imports, showProblems: showProblems) { id in
                            Task { await browser.showImported(id) }
                        }
                    }
                    if let sessionError {
                        Text(sessionError).font(.subheadline).foregroundStyle(.secondary)
                    }
                    if let failure = browser.failure, !browser.items.isEmpty {
                        VStack(spacing: 8) {
                            Text(failure).font(.subheadline)
                            Button("Retry") { Task { await browser.refresh() } }
                        }
                    }
                    if browser.isLocating {
                        ProgressView("Finding Imported Sticker…")
                    }
                    content
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .frame(maxWidth: .infinity)
            }
            .refreshable { await refresh() }
            .scrollDismissesKeyboard(.interactively)
            .scrollPosition(id: $browser.visibleStickerID)
            .onChange(of: browser.scrollTarget) { _, target in
                guard let target else { return }
                withAnimation(reduceMotion ? nil : .default) { proxy.scrollTo(target, anchor: .center) }
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if browser.items.isEmpty {
            if browser.isLoading {
                ProgressView().frame(maxWidth: .infinity, minHeight: 240)
            } else if let error = browser.failure {
                ContentUnavailableView {
                    Label("Couldn’t Load Stickers", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Retry") { Task { await browser.refresh() } }
                }
            } else if !browser.searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                ContentUnavailableView.search(text: browser.searchText)
            } else {
                ContentUnavailableView {
                    Label(browser.filter == .starred ? "No Favorites" : "No Stickers", systemImage: "photo.on.rectangle")
                } description: {
                    Text(emptyMessage)
                } actions: {
                    if browser.filter == .all || browser.filter == .recentlyUsed {
                        Button("Import Images", systemImage: "plus", action: importImages)
                            .disabled(imports?.isRunning != false)
                    }
                }
            }
        } else {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 8)], spacing: 8) {
                ForEach(browser.items) { item in
                    StickerThumbnailView(item: item, store: thumbnails) { open(item) }
                        .matchedTransitionSource(id: StickerDetailRoute(item), in: transitionNamespace)
                        .id(item.id)
                        .onAppear {
                            if browser.items.suffix(12).contains(where: { $0.id == item.id }), browser.pageError == nil {
                                Task { await browser.loadMore() }
                            }
                        }
                }
            }
            .scrollTargetLayout()
            if browser.isLoadingMore {
                ProgressView().padding()
            } else if let error = browser.pageError {
                VStack(spacing: 8) {
                    Text(error).font(.subheadline).foregroundStyle(.secondary)
                    Button("Retry Loading More") { Task { await browser.loadMore() } }
                }
            } else if browser.hasMore {
                // A visible footer also handles a first page shorter than the viewport.
                ProgressView()
                    .task(id: browser.items.count) { await browser.loadMore() }
            }
        }
    }

    private var emptyMessage: LocalizedStringKey {
        switch browser.filter {
        case .starred: "Your favorite stickers will appear here."
        case .collection: "This collection has no stickers."
        case .tag: "This tag has no stickers."
        case .all, .recentlyUsed: "Import images from Photos or Files."
        }
    }
}
