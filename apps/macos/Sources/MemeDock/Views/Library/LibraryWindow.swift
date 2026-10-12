import MemeDockBridge
import SwiftUI

/// The library window: sections on the left, the grid in the middle, and one
/// right column that is either the open sticker or the batch the picking is
/// building up.
struct LibraryWindow: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var searchFocused: Bool
    @State private var sidebarVisibility: NavigationSplitViewVisibility = .all
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
        NavigationSplitView(columnVisibility: $sidebarVisibility) {
            LibrarySidebar(model: model)
                .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 300)
                .toolbar(removing: .sidebarToggle)
        } detail: {
            VStack(spacing: 0) {
                header
                HStack(spacing: 0) {
                    StickerGrid(model: model)
                    // One column, one job: picking a batch takes it while it lasts,
                    // so an action can never land on the single open sticker by
                    // mistake.
                    if model.selecting {
                        BatchPane(model: model)
                            .frame(width: 320)
                            .transition(.opacity)
                    } else if detail.id != nil || detail.loading || detail.failure != nil {
                        StickerDetailPane(model: model)
                            .frame(width: 320)
                            .transition(.opacity)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .background(MemeDockPalette(scheme).canvas)
            .toolbar(removing: .sidebarToggle)
            .animation(reduceMotion ? nil : MemeDockMotion.fade, value: model.selecting)
        }
        .focusedSceneValue(\.librarySearchFocus, { searchFocused = true })
        .toolbar(removing: .sidebarToggle)
        .toolbar {
            if #available(macOS 26.0, *) {
                ToolbarItem(placement: .navigation) { sidebarToggle }
                    .sharedBackgroundVisibility(.hidden)
            } else {
                ToolbarItem(placement: .navigation) { sidebarToggle }
            }
        }
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

    private var sidebarToggle: some View {
        let expanded = sidebarVisibility != .detailOnly
        return Button {
            withAnimation(reduceMotion ? nil : MemeDockMotion.panel) {
                sidebarVisibility = expanded ? .detailOnly : .all
            }
        } label: {
            Image(systemName: "sidebar.left")
                .foregroundStyle(expanded ? MemeDockPalette(scheme).text : MemeDockPalette(scheme).secondary)
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(L10n.text(expanded ? .hideSidebar : .showSidebar))
        .accessibilityLabel(L10n.text(expanded ? .hideSidebar : .showSidebar))
    }

    private var header: some View {
        HStack(spacing: 12) {
            searchField
            Menu {
                Picker(L10n.text(.sortHint), selection: $browser.sort) {
                    Text(L10n.text(.sortRecent)).tag(StickerSort.recent)
                    if browser.filter.allowsCollectionOrder {
                        Text(L10n.text(.sortCollectionOrder)).tag(StickerSort.collectionOrder)
                    }
                    Text(L10n.text(.sortLastUsed)).tag(StickerSort.lastUsed)
                }
                .pickerStyle(.inline)
            } label: {
                HStack(spacing: 6) {
                    Text(sortLabel)
                        .font(MemeDockTypography.control)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 10, weight: .medium))
                }
                .frame(height: MemeDockGeometry.controlHeight)
                .contentShape(Rectangle())
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .help(L10n.text(.sortHint))
            .fixedSize()
            Button { model.setSelecting(!model.selecting) } label: {
                Label(L10n.text(.selectItems), systemImage: "checkmark.circle")
                    .labelStyle(.iconOnly)
                    .frame(width: MemeDockGeometry.controlHeight, height: MemeDockGeometry.controlHeight)
            }
            .buttonStyle(.plain)
            .foregroundStyle(model.selecting ? MemeDockPalette(scheme).text : MemeDockPalette(scheme).secondary)
            .accessibilityAddTraits(model.selecting ? .isSelected : [])
            .font(MemeDockTypography.control)
            .fixedSize()
            .help(L10n.text(.selectItems))
            Button {
                imports.presentPicker(collectionId: model.importCollectionId)
            } label: {
                Label(L10n.text(.libraryAdd), systemImage: "plus")
                    .labelStyle(.iconOnly)
                    .frame(width: MemeDockGeometry.controlHeight, height: MemeDockGeometry.controlHeight)
            }
            .keyboardShortcut("o")
            .buttonStyle(.plain)
            .disabled(imports.isRunning)
            .help(L10n.text(.importAction))
        }
        .padding(.horizontal, MemeDockGeometry.pageInset)
        .padding(.top, 12)
        .padding(.bottom, 12)
        .background(MemeDockPalette(scheme).canvas)
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
                .tint(palette.accent)
                .onExitCommand {
                    if browser.searchText.isEmpty {
                        searchFocused = false
                    } else {
                        browser.searchText = ""
                    }
                }
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
        .frame(height: MemeDockGeometry.controlHeight)
        .background(palette.input,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .overlay {
            RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                .strokeBorder(searchFocused ? palette.text.opacity(0.3) : .clear, lineWidth: 1)
        }
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: searchFocused)
    }

    private var sortLabel: String {
        switch browser.effectiveSort {
        case .recent: return L10n.text(.sortRecent)
        case .collectionOrder: return L10n.text(.sortCollectionOrder)
        case .lastUsed: return L10n.text(.sortLastUsed)
        }
    }

}
