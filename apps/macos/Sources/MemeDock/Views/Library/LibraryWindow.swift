import MemeDockBridge
import SwiftUI

/// The library window: sections on the left, the grid in the middle, and one
/// right column that is either the open sticker or the batch the picking is
/// building up.
struct LibraryWindow: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var searchFocused: Bool
    @ObservedObject var model: LibraryModel
    /// Observed so a request from the settings window is answered as soon as it
    /// arrives, even while nothing else about the window changes.
    @ObservedObject var navigation: LibraryNavigation
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var imports: ImportCoordinator

    init(model: LibraryModel, navigation: LibraryNavigation) {
        self.model = model
        self.navigation = navigation
        _browser = ObservedObject(wrappedValue: model.browser)
        _detail = ObservedObject(wrappedValue: model.detail)
        _imports = ObservedObject(wrappedValue: model.imports)
    }

    var body: some View {
        NavigationSplitView {
            LibrarySidebar(model: model)
                .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 300)
        } detail: {
            VStack(spacing: 0) {
                header
                HStack(spacing: 0) {
                    StickerGrid(model: model)
                    // One column, one job: picking a batch takes it while it lasts,
                    // so an action can never land on the single open sticker by
                    // mistake.
                    if model.selecting {
                        Divider()
                        BatchPane(model: model)
                            .frame(width: 320)
                            .transition(.opacity)
                    } else if detail.id != nil || detail.loading || detail.failure != nil {
                        Divider()
                        StickerDetailPane(model: model)
                            .frame(width: 320)
                            .transition(.opacity)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .background(MemeDockPalette(scheme).canvas)
            .animation(reduceMotion ? nil : MemeDockMotion.fade, value: model.selecting)
        }
        .focusedSceneValue(\.librarySearchFocus, { searchFocused = true })
        .toolbarBackground(.hidden, for: .windowToolbar)
        // What makes ⌘C and ⌘A land on the library instead of nowhere.
        .background { LibraryResponderInstaller(model: model) }
        // Where the settings window finds this one when it has to bring it
        // forward, and what lets it ask for a section at all.
        .background { HostWindowReader(host: navigation.library) }
        .onChange(of: navigation.request) { _, request in
            guard let request else { return }
            model.select(request.section)
        }
        .task { await model.start() }
        .onDisappear { model.shutdown() }
    }

    private var search: Binding<String> {
        Binding(
            get: { browser.searchText },
            set: { browser.searchText = $0 }
        )
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .center, spacing: 12) {
                Text(sectionTitle)
                    .font(MemeDockTypography.pageTitle)
                    .lineLimit(1)
                Spacer(minLength: 12)
                Button {
                    imports.presentPicker(collectionId: model.importCollectionId)
                } label: {
                    Label(L10n.text(.libraryAdd), systemImage: "plus")
                }
                .keyboardShortcut("o")
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                .disabled(imports.isRunning)
                .help(L10n.text(.importAction))
            }
            HStack(spacing: 12) {
                searchField
                Picker(L10n.text(.sortHint), selection: $browser.sort) {
                    Text(L10n.text(.sortRecent)).tag(StickerSort.recent)
                    if browser.filter.allowsCollectionOrder {
                        Text(L10n.text(.sortCollectionOrder)).tag(StickerSort.collectionOrder)
                    }
                    Text(L10n.text(.sortLastUsed)).tag(StickerSort.lastUsed)
                }
                .pickerStyle(.menu)
                .font(MemeDockTypography.control)
                .fixedSize()
                Toggle(isOn: selecting) {
                    Label(L10n.text(.selectItems), systemImage: "checkmark.circle")
                }
                .toggleStyle(.button)
                .font(MemeDockTypography.control)
                .fixedSize()
                .help(L10n.text(.selectItems))
            }
        }
        .padding(.horizontal, MemeDockGeometry.pageInset)
        .padding(.top, 12)
        .padding(.bottom, 16)
        .background(MemeDockGlass())
    }

    private var searchField: some View {
        let palette = MemeDockPalette(scheme)
        return HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(palette.secondary)
            TextField(L10n.text(.search), text: search)
                .textFieldStyle(.plain)
                .font(MemeDockTypography.body)
                .focused($searchFocused)
            if !browser.searchText.isEmpty {
                Button { browser.searchText = "" } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.plain)
                .foregroundStyle(palette.secondary)
                .help(L10n.text(.clearSearch))
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 36)
        .background(palette.input.opacity(0.72),
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .overlay {
            RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                .strokeBorder(searchFocused ? palette.accent.opacity(0.45) : .clear, lineWidth: 1)
        }
    }

    private var sectionTitle: String {
        switch browser.filter {
        case .all: return L10n.text(.all)
        case .starred: return L10n.text(.starred)
        case .recentlyUsed: return L10n.text(.recentlyUsed)
        case .deleted: return L10n.text(.trash)
        case let .collection(id):
            return model.collections.active.first { $0.id == id }?.name ?? L10n.text(.collections)
        case let .tag(id):
            return model.tags.active.first { $0.id == id }?.name ?? L10n.text(.tags)
        }
    }

    /// Leaving the mode drops what was picked: a selection that is no longer
    /// shown anywhere would keep rows tinted with nothing left to act on.
    private var selecting: Binding<Bool> {
        Binding(
            get: { model.selecting },
            set: { model.setSelecting($0) }
        )
    }
}
