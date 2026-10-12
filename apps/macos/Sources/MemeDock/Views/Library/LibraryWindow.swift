import MemeDockBridge
import SwiftUI

/// The library window: sections on the left, the grid in the middle, and one
/// right column that is either the open sticker or the batch the picking is
/// building up.
struct LibraryWindow: View {
    @ObservedObject var model: LibraryModel
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var imports: ImportCoordinator

    init(model: LibraryModel) {
        self.model = model
        _browser = ObservedObject(wrappedValue: model.browser)
        _detail = ObservedObject(wrappedValue: model.detail)
        _imports = ObservedObject(wrappedValue: model.imports)
    }

    var body: some View {
        NavigationSplitView {
            LibrarySidebar(model: model)
                .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 300)
        } detail: {
            HStack(spacing: 0) {
                StickerGrid(model: model)
                // One column, one job: picking a batch takes it while it lasts,
                // so an action can never land on the single open sticker by
                // mistake.
                if model.selecting {
                    Divider()
                    BatchPane(model: model)
                        .frame(width: 320)
                } else if detail.id != nil || detail.loading || detail.failure != nil {
                    Divider()
                    StickerDetailPane(model: model)
                        .frame(width: 320)
                }
            }
        }
        .searchable(text: search, placement: .toolbar, prompt: Text(L10n.text(.search)))
        .toolbar { toolbar }
        // What makes ⌘C and ⌘A land on the library instead of nowhere.
        .background { LibraryResponderInstaller(model: model) }
        .task { await model.start() }
        .onDisappear { model.shutdown() }
    }

    private var search: Binding<String> {
        Binding(
            get: { browser.searchText },
            set: { browser.searchText = $0 }
        )
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .primaryAction) {
            Button {
                imports.presentPicker(collectionId: model.importCollectionId)
            } label: {
                Label(L10n.text(.libraryAdd), systemImage: "plus")
            }
            .keyboardShortcut("o")
            .disabled(imports.isRunning)
            .help(L10n.text(.importAction))
        }
        ToolbarItem {
            Picker(L10n.text(.sortHint), selection: $browser.sort) {
                Text(L10n.text(.sortRecent)).tag(StickerSort.recent)
                if browser.filter.allowsCollectionOrder {
                    Text(L10n.text(.sortCollectionOrder)).tag(StickerSort.collectionOrder)
                }
                Text(L10n.text(.sortLastUsed)).tag(StickerSort.lastUsed)
            }
            .pickerStyle(.menu)
        }
        ToolbarItem {
            Toggle(isOn: selecting) {
                Label(L10n.text(.selectItems), systemImage: "checkmark.circle")
            }
            .help(L10n.text(.selectItems))
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
