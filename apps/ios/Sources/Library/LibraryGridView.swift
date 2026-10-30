import SwiftUI

struct LibraryGridView: View {
    let browser: LibraryBrowser
    let thumbnails: ThumbnailStore
    let imports: ImportCoordinator?
    var session: LibrarySession? = nil
    let sessionError: String?
    let refresh: () async -> Void
    let showProblems: () -> Void
    let importImages: () -> Void
    let hero: StickerHero
    let open: (LibraryItem, CGImage?) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        @Bindable var browser = browser
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 12, pinnedViews: [.sectionHeaders]) {
                    Section {
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
                    } header: {
                        if let imports {
                            VStack(spacing: 8) {
                                homeControls
                                if imports.isRunning || imports.hasProblems {
                                    ImportStatusView(coordinator: imports, showProblems: showProblems)
                                        .transition(
                                            reduceMotion ? .opacity : .move(edge: .top).combined(with: .opacity))
                                }
                            }
                            .padding(.top, 4).padding(.bottom, 4)
                            .background(Color(uiColor: .systemGroupedBackground))
                            .animation(
                                reduceMotion ? nil : .snappy, value: imports.isRunning || imports.hasProblems)
                        }
                    }
                }
                .padding(.horizontal, 16)
                .frame(maxWidth: .infinity)
            }
            .refreshable { await refresh() }
            .scrollDismissesKeyboard(.interactively)
            .background(Color(uiColor: .systemGroupedBackground))
            .scrollPosition(id: $browser.visibleStickerID)
            .onChange(of: browser.scrollTarget) { _, target in
                guard let target else { return }
                withAnimation(reduceMotion ? nil : .default) { proxy.scrollTo(target, anchor: .center) }
            }
        }
    }

    private var homeControls: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                filterButton("Recent", value: .recentlyUsed)
                filterButton("All", value: .all)
                filterButton("Favorites", value: .starred)
                ForEach(browser.collections, id: \.id) { collection in
                    Button {
                        browser.filter = .collection(collection.id)
                    } label: {
                        Label(collection.name, systemImage: "folder")
                            .font(.subheadline).lineLimit(1).padding(.horizontal, 12).padding(.vertical, 8)
                            .background(
                                browser.filter == .collection(collection.id)
                                    ? Color.accentColor.opacity(0.14) : Color(uiColor: .tertiarySystemFill),
                                in: .capsule)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(browser.filter == .collection(collection.id) ? .isSelected : [])
                }
            }
        }.scrollIndicators(.hidden)
    }

    private func filterButton(_ title: LocalizedStringKey, value: LibraryFilter) -> some View {
        Button {
            browser.filter = value
        } label: {
            Text(title).font(.subheadline).padding(.horizontal, 12).padding(.vertical, 8)
                .foregroundStyle(browser.filter == value ? Color.accentColor : .primary)
                .background(
                    browser.filter == value ? Color.accentColor.opacity(0.14) : Color(uiColor: .tertiarySystemFill),
                    in: .capsule)
        }.buttonStyle(.plain).accessibilityAddTraits(browser.filter == value ? .isSelected : [])
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
                    Label(
                        browser.filter == .starred ? "No Favorites" : "No Stickers", systemImage: "photo.on.rectangle")
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
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 4)], spacing: 4) {
                ForEach(browser.items) { item in
                    StickerThumbnailView(item: item, store: thumbnails, hero: hero) { image in
                        if browser.isSelecting { browser.toggleSelection(item) } else { open(item, image) }
                    }
                    .scaleEffect(browser.selected[item.id] != nil ? 0.9 : 1)
                    .background(
                        browser.selected[item.id] != nil ? Color.accentColor.opacity(0.12) : .clear,
                        in: .rect(cornerRadius: 12)
                    )
                    .animation(reduceMotion ? nil : .snappy(duration: 0.2), value: browser.selected[item.id] != nil)
                    .overlay(alignment: .topTrailing) {
                        if browser.isSelecting {
                            Image(systemName: browser.selected[item.id] != nil ? "checkmark.circle.fill" : "circle")
                                .font(.title2).symbolRenderingMode(.palette)
                                .foregroundStyle(.white, Color.accentColor)
                                .contentTransition(.symbolEffect(.replace))
                                .animation(reduceMotion ? nil : .snappy(duration: 0.2), value: browser.selected[item.id] != nil)
                                .padding(6).allowsHitTesting(false)
                        } else if item.sticker.starred {
                            Image(systemName: "star.fill").foregroundStyle(.orange).padding(6).allowsHitTesting(false)
                        }
                    }
                    .overlay(alignment: .bottomLeading) {
                        if item.resource?.asset.animated == true {
                            Image(systemName: "play.fill").font(.caption2).padding(5)
                                .background(.thinMaterial, in: .capsule).padding(4).allowsHitTesting(false)
                        }
                    }
                    .accessibilityAddTraits(browser.selected[item.id] != nil ? .isSelected : [])
                    .contextMenu {
                        if !browser.isSelecting {
                            Section {
                                if let outputs = session?.outputs, let preferences = session?.outputPreferences,
                                    !outputs.isBusy
                                {
                                    Button("Copy", systemImage: "doc.on.doc") {
                                        outputs.start(
                                            .copy, sticker: item.id,
                                            preset: preferences.choice(
                                                .copy, animated: item.resource?.asset.animated == true))
                                    }
                                    if item.resource?.asset.animated == true {
                                        Menu {
                                            Button {
                                                outputs.start(
                                                    .share, sticker: item.id,
                                                    preset: preferences.choice(.share, animated: true))
                                            } label: {
                                                Text(preferences.choice(.share, animated: true).title)
                                            }
                                            Button("Share First Frame as PNG", systemImage: "photo") {
                                                outputs.start(.share, sticker: item.id, preset: .best, firstFrame: true)
                                            }
                                        } label: {
                                            Label("Share", systemImage: "square.and.arrow.up")
                                        }
                                    } else {
                                        Button("Share", systemImage: "square.and.arrow.up") {
                                            outputs.start(
                                                .share, sticker: item.id,
                                                preset: preferences.choice(.share, animated: false))
                                        }
                                    }
                                }
                            }
                            Section {
                                Button(
                                    item.sticker.starred ? "Remove Favorite" : "Favorite",
                                    systemImage: item.sticker.starred ? "star.slash" : "star"
                                ) {
                                    Task {
                                        _ = await browser.apply(.star(value: !item.sticker.starred), to: [item.sticker])
                                    }
                                }
                                Button("Organize", systemImage: "tag") {
                                    browser.startSelection()
                                    browser.toggleSelection(item)
                                    browser.isPresentingActions = true
                                }
                                Button("Select", systemImage: "checkmark.circle") {
                                    browser.startSelection()
                                    browser.toggleSelection(item)
                                }
                            }
                            Section {
                                Button("Move to Trash", systemImage: "trash", role: .destructive) {
                                    Task {
                                        if await browser.apply(.delete, to: [item.sticker]) {
                                            session?.feedback.post(String(localized: "Moved to Trash"), action: .viewTrash)
                                        }
                                    }
                                }
                            }
                        }
                    }
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
