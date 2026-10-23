import SwiftUI
import PhotosUI
import UniformTypeIdentifiers

struct RootView: View {
    let session: LibrarySession
    @State private var showPhotos = false
    @State private var showFiles = false
    @State private var showProblems = false
    @State private var showImportSources = false
    @State private var selectedPhotos: [PhotosPickerItem] = []

    @State private var navigationPath: [StickerDetailRoute] = []
    @Namespace private var transitionNamespace
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        NavigationStack(path: $navigationPath) {
            Group {
                switch session.phase {
                case .idle, .opening, .closing:
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                case .ready:
                    if let browser = session.browser, let thumbnails = session.thumbnails {
                        libraryView(browser, thumbnails: thumbnails)
                    }
                case .failed(let error):
                    ContentUnavailableView {
                        Label("Library Unavailable", systemImage: "exclamationmark.triangle")
                    } description: {
                        Text(error)
                    } actions: {
                        Button {
                            Task { await session.retry() }
                        } label: {
                            Label("Retry", systemImage: "arrow.clockwise")
                        }
                    }
                case .closed:
                    ContentUnavailableView("Library Closed", systemImage: "externaldrive")
                }
            }
            .navigationTitle(session.browser?.filterTitle ?? String(localized: "Stickers"))
            .navigationDestination(for: StickerDetailRoute.self) { route in
                if reduceMotion {
                    StickerDetailView(session: session, route: route)
                } else {
                    StickerDetailView(session: session, route: route)
                        .navigationTransition(.zoom(sourceID: route, in: transitionNamespace))
                }
            }
            .toolbar {
                if session.phase == .ready, navigationPath.isEmpty {
                    ToolbarItem(placement: .primaryAction) { importMenu }
                }
            }
            .confirmationDialog("Import Images", isPresented: $showImportSources) {
                Button("Photos", systemImage: "photo") { showPhotos = true }
                Button("Files", systemImage: "folder") { showFiles = true }
            }
            .photosPicker(
                isPresented: $showPhotos, selection: $selectedPhotos,
                maxSelectionCount: 200, matching: .images, preferredItemEncoding: .current
            )
            .onChange(of: selectedPhotos) { _, selected in
                guard !selected.isEmpty else { return }
                session.imports?.start(selected.map(ImportSource.photo))
                selectedPhotos = []
            }
            .fileImporter(isPresented: $showFiles, allowedContentTypes: [.image], allowsMultipleSelection: true) { result in
                switch result {
                case .success(let urls): session.imports?.start(urls.map(ImportSource.file))
                case .failure(let error): session.imports?.selectionFailed(error)
                }
            }
            .sheet(isPresented: $showProblems) {
                if let imports = session.imports { ImportProblemsView(coordinator: imports) }
            }
            .alert("Couldn’t Import", isPresented: Binding(
                get: { session.imports?.selectionError != nil },
                set: { if !$0 { session.imports?.dismissSelectionError() } }
            )) {
                Button("OK") { session.imports?.dismissSelectionError() }
            } message: {
                Text(session.imports?.selectionError ?? "")
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.didReceiveMemoryWarningNotification)) { _ in
                Task { await session.thumbnails?.clearCache() }
            }
            .onChange(of: session.phase) { _, phase in
                if phase != .ready { navigationPath = [] }
            }
        }
    }

    private func libraryView(_ browser: LibraryBrowser, thumbnails: ThumbnailStore) -> some View {
        @Bindable var browser = browser
        return LibraryGridView(
            browser: browser, thumbnails: thumbnails, imports: session.imports,
            sessionError: session.queryError, refresh: { await session.refresh() },
            showProblems: { showProblems = true }, importImages: { showImportSources = true },
            transitionNamespace: transitionNamespace, open: { item in
                guard navigationPath.isEmpty else { return }
                navigationPath.append(StickerDetailRoute(item))
            }
        )
        .searchable(text: $browser.searchText, prompt: "Search Stickers")
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Menu {
                    Picker("Show", selection: $browser.filter) {
                        Text("Recently Used").tag(LibraryFilter.recentlyUsed)
                        Text("All Stickers").tag(LibraryFilter.all)
                        Text("Favorites").tag(LibraryFilter.starred)
                        ForEach(browser.collections, id: \.id) { collection in
                            Text(collection.name).tag(LibraryFilter.collection(collection.id))
                        }
                    }
                    if browser.collectionError != nil {
                        Button("Retry Loading Collections") { Task { await browser.loadCollections() } }
                    }
                } label: {
                    Label("Filter", systemImage: "line.3.horizontal.decrease")
                }
                .accessibilityValue(browser.filterTitle)
            }
        }
    }

    private var importMenu: some View {
        Menu {
            Button("Photos", systemImage: "photo") { showPhotos = true }
            Button("Files", systemImage: "folder") { showFiles = true }
        } label: {
            Label("Import", systemImage: "plus")
        }
        .disabled(session.imports?.isRunning != false)
        .accessibilityIdentifier("library-import")
    }
}
