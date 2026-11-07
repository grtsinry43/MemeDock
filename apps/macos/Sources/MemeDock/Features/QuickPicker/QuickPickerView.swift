import AppKit
import ApplicationServices
import SwiftUI

struct QuickPickerView: View {
    @ObservedObject var model: QuickPickerModel
    let dismiss: () -> Void
    let openLibrary: (String?) -> Void
    let dragChanged: (Bool, Bool) -> Void
    let requestAccess: () -> Void
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var session: LibrarySession
    @ObservedObject private var actions: StickerActions
    @ObservedObject private var status: StatusCenter
    @ObservedObject private var collections: CollectionCatalog
    @ObservedObject private var preferences = Preferences.shared
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @FocusState private var searchFocused: Bool

    init(model: QuickPickerModel, dismiss: @escaping () -> Void, openLibrary: @escaping (String?) -> Void, dragChanged: @escaping (Bool, Bool) -> Void, requestAccess: @escaping () -> Void) {
        self.model = model
        self.dismiss = dismiss
        self.openLibrary = openLibrary
        self.dragChanged = dragChanged
        self.requestAccess = requestAccess
        _browser = ObservedObject(wrappedValue: model.browser)
        _session = ObservedObject(wrappedValue: model.session)
        _actions = ObservedObject(wrappedValue: model.actions)
        _status = ObservedObject(wrappedValue: model.status)
        _collections = ObservedObject(wrappedValue: model.collections)
    }

    var body: some View {
        VStack(spacing: 0) {
            search
            filters
            Divider().opacity(0.4)
            content
                .overlay(alignment: .bottom) {
                    if model.activity == nil, !actions.busy, let message = status.message {
                        FeedbackBanner(
                            message: message, isFailure: status.isFailure, dismiss: status.clear,
                            actionTitle: needsAccess ? L10n.text(.grantAccessibility) : nil,
                            action: needsAccess ? requestAccess : nil
                        )
                        .frame(height: needsAccess ? 88 : 64)
                        .padding(.horizontal, 12).padding(.bottom, 8)
                    }
                }
            footer
        }
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).text)
        .background {
            if reduceTransparency { MemeDockPalette(scheme).canvas }
            else { PickerMaterial().overlay(MemeDockPalette(scheme).canvas.opacity(0.35)) }
        }
        .clipShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .defaultFocus($searchFocused, true)
        .task(id: model.presentation) { searchFocused = true }
        .onChange(of: browser.items.map(\.id)) { _, ids in
            if !ids.contains(model.focusedId ?? "") { model.focusedId = ids.first }
        }
        .onExitCommand(perform: dismiss)
    }

    private var search: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField(L10n.text(.searchStickers), text: $browser.searchText)
                .textFieldStyle(.plain)
                .focused($searchFocused)
                .onSubmit { if let item = model.focusedItem { model.activate(item) } }
                .accessibilityLabel(L10n.text(.searchStickers))
            if !browser.searchText.isEmpty {
                Button { browser.searchText = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .help(L10n.text(.clearSearch))
            }
            Button { openLibrary(nil) } label: { Image(systemName: "arrow.up.right.square") }
                .help(L10n.text(.openInLibrary))
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 16)
        .frame(height: 48)
    }

    private var filters: some View {
        HStack(spacing: 4) {
            filter(.recentlyUsed, .recentlyUsed)
            filter(.starred, .starred)
            filter(.all, .pickerAll)
            Spacer(minLength: 4)
            Menu {
                ForEach(collections.active, id: \.id) { collection in
                    Button(collection.name) { model.select(.collection(collection.id)) }
                }
                if let failure = collections.failure {
                    Text(failure)
                    Button(L10n.text(.retry)) { Task { await collections.load() } }
                }
            } label: {
                if case let .collection(id) = browser.filter,
                   let collection = collections.active.first(where: { $0.id == id }) {
                    Text(collection.name).lineLimit(1)
                } else { Text(L10n.text(.collections)) }
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .frame(maxWidth: 135)
            .disabled(collections.active.isEmpty && collections.failure == nil)
        }
        .padding(.horizontal, 12)
        .padding(.bottom, 10)
    }

    private func filter(_ filter: LibraryFilter, _ title: L10nKey) -> some View {
        let selected = browser.filter == filter
        let palette = MemeDockPalette(scheme)
        return Button { model.select(filter) } label: {
            Text(L10n.text(title))
                .font(MemeDockTypography.caption)
                .padding(.horizontal, 9)
                .padding(.vertical, 5)
                .foregroundStyle(selected ? palette.raised : palette.text)
                .background {
                    if selected { RoundedRectangle(cornerRadius: MemeDockGeometry.control).fill(palette.text) }
                }
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    @ViewBuilder
    private var content: some View {
        if case let .failed(message) = session.phase {
            empty(message, retry: model.retry)
        } else if session.phase != .ready || browser.isLoading && browser.items.isEmpty {
            ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if browser.items.isEmpty {
            if let failure = browser.failure { empty(failure, retry: browser.reload) }
            else { empty(emptyMessage) }
        } else {
            grid
        }
    }

    private var grid: some View {
        ScrollViewReader { scroller in
            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: model.columns), spacing: 8) {
                    ForEach(browser.items) { item in
                        QuickPickerTile(item: item, model: model, selected: model.focusedId == item.id,
                                        openLibrary: openLibrary, dragChanged: dragChanged)
                            .id(item.id)
                    }
                }
                .padding(12)
                if let failure = browser.failure {
                    empty(failure, retry: browser.loadMore)
                } else if browser.hasMore {
                    ProgressView().controlSize(.small).padding(8)
                        .onAppear { browser.loadMore() }
                }
            }
            .onChange(of: model.focusedId) { _, id in
                if let id { scroller.scrollTo(id, anchor: .center) }
            }
        }
    }

    private var emptyMessage: String {
        if !browser.searchText.isEmpty { return L10n.text(.noResults) }
        switch browser.filter {
        case .starred: return L10n.text(.starredEmpty)
        case .collection: return L10n.text(.collectionEmpty)
        default: return L10n.text(.emptyLibrary)
        }
    }

    private func empty(_ message: String, retry: (() -> Void)? = nil) -> some View {
        VStack(spacing: 10) {
            Text(message).multilineTextAlignment(.center).foregroundStyle(.secondary)
            if let retry { Button(L10n.text(.retry), action: retry) }
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: 8) {
            if model.activity != nil || actions.busy || browser.isLoading {
                ProgressView().controlSize(.mini)
                Text(L10n.text(model.activity ?? (actions.busy ? .quickPickerPreparing : .loading)))
            }
            Spacer(minLength: 0)
            if let item = model.focusedItem {
                Button { model.activate(item) } label: {
                    Label(L10n.text(.pasteImage), systemImage: "return")
                }
                .buttonStyle(.plain).disabled(!model.acceptsInput)
            }
        }
        .font(MemeDockTypography.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 14)
        .frame(height: 30)
    }

    private var needsAccess: Bool {
        status.isFailure && (!AXIsProcessTrusted() || !CGPreflightPostEventAccess())
    }
}

struct QuickPickerTile: View {
    let item: StickerItem
    let model: QuickPickerModel
    let selected: Bool
    let openLibrary: (String?) -> Void
    let dragChanged: (Bool, Bool) -> Void
    @ObservedObject private var thumbnails: ThumbnailStore
    @ObservedObject private var actions: StickerActions
    @Environment(\.colorScheme) private var scheme

    init(item: StickerItem, model: QuickPickerModel, selected: Bool, openLibrary: @escaping (String?) -> Void, dragChanged: @escaping (Bool, Bool) -> Void) {
        self.item = item
        self.model = model
        self.selected = selected
        self.openLibrary = openLibrary
        self.dragChanged = dragChanged
        _thumbnails = ObservedObject(wrappedValue: model.thumbnails)
        _actions = ObservedObject(wrappedValue: model.actions)
    }

    var body: some View {
        Button { model.activate(item) } label: {
            ZStack {
                if let image = thumbnails.image(for: item) {
                    Image(nsImage: image).resizable().scaledToFit().padding(6)
                } else if thumbnails.failed.contains(item.id) {
                    Image(systemName: "photo.badge.exclamationmark").foregroundStyle(.secondary)
                } else { ProgressView().controlSize(.small) }
            }
            .frame(maxWidth: .infinity)
            .frame(height: 76)
            .background(selected ? MemeDockPalette(scheme).text.opacity(0.10) : .clear,
                        in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(actions.busy || item.asset == nil)
        .overlay {
            QuickPickerDragSource(item: item, model: model, openLibrary: openLibrary, dragChanged: dragChanged)
        }
        .help(item.sticker.title.isEmpty ? L10n.text(.pasteImage) : item.sticker.title)
        .accessibilityLabel(item.sticker.title.isEmpty ? L10n.text(.pasteImage) : item.sticker.title)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .onAppear { thumbnails.load(item) }
        .onChange(of: item.thumbnailPath) { _, _ in thumbnails.load(item) }
        .onChange(of: item.thumbnailStatus) { _, _ in thumbnails.load(item) }
    }
}

private struct PickerMaterial: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .popover
        view.blendingMode = .behindWindow
        view.state = .active
        return view
    }
    func updateNSView(_ view: NSVisualEffectView, context: Context) {}
}
