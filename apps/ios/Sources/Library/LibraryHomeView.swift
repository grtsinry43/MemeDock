import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

struct LibraryHomeView: View {
    @Bindable var navigation: AppNavigation
    let session: LibrarySession
    @State private var showPhotos = false
    @State private var showFiles = false
    @State private var showProblems = false
    @State private var showImportSources = false
    @State private var showTelegram = false
    @State private var selectedPhotos: [PhotosPickerItem] = []

    var body: some View {
        NavigationStack {
            Group {
                if let browser = session.browser, let thumbnails = session.thumbnails, let organize = session.organize {
                    libraryView(browser, thumbnails: thumbnails, organize: organize)
                }
            }
            .navigationTitle("Stickers")
            .toolbar {
                if session.phase == .ready {
                    ToolbarItem(placement: .primaryAction) { importMenu }
                }
            }
            .navigationDestination(isPresented: $showTelegram) {
                if let telegram = session.telegram { TelegramView(model: telegram).toolbar(.hidden, for: .tabBar) }
            }
            .confirmationDialog("Import Images", isPresented: $showImportSources) {
                Button("Photos", systemImage: "photo") { showPhotos = true }
                Button("Files", systemImage: "folder") { showFiles = true }
                Button("Telegram", systemImage: "paperplane") { showTelegram = true }
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
            .fileImporter(isPresented: $showFiles, allowedContentTypes: [.image], allowsMultipleSelection: true) {
                result in
                switch result {
                case .success(let urls): session.imports?.start(urls.map(ImportSource.file))
                case .failure(let error): session.imports?.selectionFailed(error)
                }
            }
            .sheet(isPresented: $showProblems) {
                if let imports = session.imports {
                    ImportProblemsView(coordinator: imports) {
                        navigation.selected = .mine
                        navigation.minePath = [.trash]
                    }
                }
            }
            .alert(
                "Couldn’t Import",
                isPresented: Binding(
                    get: { session.imports?.selectionError != nil },
                    set: { if !$0 { session.imports?.dismissSelectionError() } }
                )
            ) {
                Button("OK") { session.imports?.dismissSelectionError() }
            } message: {
                Text(session.imports?.selectionError ?? "")
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.didReceiveMemoryWarningNotification)) {
                _ in
                Task { await session.thumbnails?.clearCache() }
            }
        }
    }

    private func libraryView(_ browser: LibraryBrowser, thumbnails: ThumbnailStore, organize: OrganizeModel)
        -> some View
    {
        @Bindable var browser = browser
        return LibraryGridView(
            browser: browser, thumbnails: thumbnails, imports: session.imports, session: session,
            sessionError: session.queryError, refresh: { await session.refresh() },
            showProblems: { showProblems = true }, importImages: { showImportSources = true },
            hero: navigation.hero,
            open: { item, image in navigation.hero.present(StickerDetailRoute(item), image: image) }
        )
        .modifier(StickerActionsToolbar(browser: browser, organize: organize))
        .searchable(
            text: $browser.searchText, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search Stickers")
    }

    private var importMenu: some View {
        Menu {
            Button("Photos", systemImage: "photo") { showPhotos = true }
            Button("Files", systemImage: "folder") { showFiles = true }
            Button("Telegram", systemImage: "paperplane") { showTelegram = true }
        } label: {
            Label("Import", systemImage: "plus")
        }
        .labelStyle(.iconOnly)
        .disabled(session.imports?.isRunning != false)
        .accessibilityIdentifier("library-import")
    }
}
