import AppKit
import MemeDockBridge
import SwiftUI

/// The grid of the current section. Paging belongs to the browser: a row near
/// the end asks for the next page, so scrolling keeps flowing.
struct StickerGrid: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var model: LibraryModel
    let hero: HeroFlight
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var selection: StickerSelection
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var imports: ImportCoordinator
    /// Read for the tile size, which the View menu changes.
    @ObservedObject private var preferences = Preferences.shared

    private static let spacing: CGFloat = 12
    private static let inset: CGFloat = 16

    init(model: LibraryModel, hero: HeroFlight) {
        self.model = model
        self.hero = hero
        _browser = ObservedObject(wrappedValue: model.browser)
        _selection = ObservedObject(wrappedValue: model.selection)
        _detail = ObservedObject(wrappedValue: model.detail)
        _imports = ObservedObject(wrappedValue: model.imports)
    }

    var body: some View {
        VStack(spacing: 0) {
            if imports.isRunning { importStrip }
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private var content: some View {
        if browser.isLoading, browser.items.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if browser.items.isEmpty {
            // Nothing is on screen, so a failure has the whole area.
            if let failure = browser.failure {
                failureState(failure)
            } else {
                emptyState
            }
        } else {
            grid
        }
    }

    private var grid: some View {
        // Only the column count comes from the measured width; flexible
        // columns then share whatever the scroll view leaves, scroll bar
        // included, so a row always fills exactly.
        GeometryReader { proxy in
            let gridColumns = columns(for: proxy.size.width)
            ScrollViewReader { scroller in
                ScrollView {
                    LazyVGrid(columns: gridColumns, spacing: Self.spacing) {
                        ForEach(browser.items) { item in
                            StickerCell(
                                item: item,
                                selected: selection.isSelected(item.id),
                                cursor: model.focusedId == item.id,
                                detailOpen: detail.id != nil || detail.loading || detail.failure != nil,
                                model: model,
                                hero: hero,
                                actions: model.actions,
                                thumbnails: model.thumbnails
                            )
                            .id(item.id)
                        }
                        // A page that failed after rows were already on screen keeps
                        // those rows and offers a retry instead of asking again by
                        // itself, which would spin while the failure lasts.
                        if let failure = browser.failure {
                            inlineFailure(failure)
                        } else if browser.hasMore {
                            ProgressView()
                                .controlSize(.small)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 12)
                                .onAppear { browser.loadMore() }
                        }
                    }
                    .padding(Self.inset)
                }
                // Only keyboard moves scroll the grid; a click is already in view.
                .onChange(of: model.focusScroll) { _, _ in
                    guard let id = model.focusedId else { return }
                    withAnimation(reduceMotion ? nil : MemeDockMotion.panel) {
                        scroller.scrollTo(id, anchor: .center)
                    }
                }
            }
            .onChange(of: gridColumns.count) { _, count in
                model.columnCount = count
            }
            .onAppear { model.columnCount = gridColumns.count }
        }
        .heroViewport(hero)
        .dropDestination(for: URL.self) { urls, _ in
            guard !urls.isEmpty else { return false }
            let collectionId = model.importCollectionId
            Task { await model.imports.importFiles(urls, collectionId: collectionId) }
            return true
        }
    }

    private func columns(for width: CGFloat) -> [GridItem] {
        let usable = width - Self.inset * 2
        let target = preferences.tileSize
        let count = max(1, Int((usable + Self.spacing) / (target + Self.spacing)))
        return Array(repeating: GridItem(.flexible(), spacing: Self.spacing), count: count)
    }

    private var importStrip: some View {
        HStack(spacing: 10) {
            if let progress = imports.progress {
                // The importer counts files, so the strip reports which one of
                // how many is being committed right now.
                ProgressView(value: Double(progress.index), total: Double(max(progress.total, 1)))
                    .progressViewStyle(.linear)
                Text(progress.name)
                    .font(MemeDockTypography.body)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .foregroundStyle(.secondary)
                Text("\(progress.index)/\(progress.total)")
                    .font(MemeDockTypography.caption)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
                    .contentTransition(reduceMotion ? .opacity : .numericText(value: Double(progress.index)))
                    .animation(reduceMotion ? nil : .snappy(duration: 0.25), value: progress.index)
            } else {
                ProgressView()
                    .controlSize(.small)
            }
            Button(L10n.text(.cancel)) { imports.cancel() }
                .controlSize(.small)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(MemeDockPalette(scheme).canvas)
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: symbol)
                .font(.system(size: 34))
                .foregroundStyle(.tertiary)
            Text(L10n.text(title))
                .font(MemeDockTypography.sectionTitle)
            Text(L10n.text(hint))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
            if allowsImport {
                Button(L10n.text(.libraryAdd)) {
                    imports.presentPicker(collectionId: model.importCollectionId)
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
            } else if searching {
                Button(L10n.text(.clearSearch)) { browser.searchText = "" }
                    .buttonStyle(MemeDockButtonStyle(kind: .quiet))
            }
        }
        .multilineTextAlignment(.center)
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func inlineFailure(_ message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(.orange)
            Text(message)
                .lineLimit(2)
                .foregroundStyle(.secondary)
            Button(L10n.text(.retry)) { browser.loadMore() }
                .controlSize(.small)
        }
        .font(MemeDockTypography.body)
        .frame(maxWidth: .infinity)
        .padding(.vertical, 12)
    }

    private func failureState(_ message: String) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 34))
                .foregroundStyle(.orange)
            Text(L10n.text(.failedToLoad))
                .font(MemeDockTypography.sectionTitle)
            Text(message)
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
            Button(L10n.text(.retry)) { browser.reload() }
        }
        .multilineTextAlignment(.center)
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// An empty result is the search's answer when a term was typed, and the
    /// section's own wording otherwise.
    private var searching: Bool {
        !browser.searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var title: L10nKey {
        if searching { return .noResults }
        switch browser.filter {
        case .starred: return .starredEmpty
        case .deleted: return .trashEmpty
        case .collection: return .collectionEmpty
        case .tag: return .tagEmpty
        case .all, .recentlyUsed: return .libraryEmpty
        }
    }

    private var hint: L10nKey {
        if searching { return .noMatchesHint }
        switch browser.filter {
        case .starred: return .starredEmptyHint
        case .deleted: return .trashEmptyHint
        case .collection: return .collectionEmptyHint
        case .tag: return .tagEmptyHint
        case .all, .recentlyUsed: return .libraryEmptyHint
        }
    }

    private var symbol: String {
        if searching { return "magnifyingglass" }
        switch browser.filter {
        case .starred: return "star"
        case .deleted: return "trash"
        case .collection: return "square.stack"
        case .tag: return "tag"
        case .all, .recentlyUsed: return "square.grid.2x2"
        }
    }

    private var allowsImport: Bool {
        if searching { return false }
        switch browser.filter {
        case .deleted, .tag: return false
        case .all, .recentlyUsed, .starred, .collection: return true
        }
    }

}

/// One sticker in the grid. It reads its thumbnail from the store so a
/// regenerated thumbnail reaches the cell that asked for it.
private struct StickerCell: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var hovering = false
    /// Where the picture sits, for a flight that starts or ends here.
    @State private var frame = FrameBox()
    let item: StickerItem
    let selected: Bool
    /// Keyboard cursor: what ⌘C and the detail column follow.
    let cursor: Bool
    let detailOpen: Bool
    let model: LibraryModel
    let hero: HeroFlight
    @ObservedObject var actions: StickerActions
    @ObservedObject var thumbnails: ThumbnailStore

    var body: some View {
        VStack(spacing: 6) {
            preview
            if !item.sticker.title.isEmpty {
                Text(item.sticker.title)
                    .font(MemeDockTypography.caption)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(6)
        .background(background, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        // A tap view, not a Button: the cursor is the grid's focus model.
        .onTapGesture { open() }
        .onHover { hovering = $0 }
        .stickerDragSource(actions, item: item)
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: selected)
        .contextMenu { menu }
        .help(item.sticker.title.isEmpty ? item.sticker.originalName : item.sticker.title)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(item.sticker.title.isEmpty ? item.sticker.originalName : item.sticker.title)
        .accessibilityAddTraits(.isButton)
        .accessibilityAddTraits(selected || cursor ? .isSelected : [])
        .accessibilityAction { open() }
    }

    private var preview: some View {
        // Square, so the picture grows with the tile.
        Color.clear
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                if let image = thumbnails.image(for: item) {
                    Image(nsImage: image)
                        .resizable()
                        .interpolation(.high)
                        .scaledToFit()
                        .padding(4)
                } else if thumbnails.failed.contains(item.id) {
                    Image(systemName: "photo")
                        .foregroundStyle(.tertiary)
                } else {
                    ProgressView()
                        .controlSize(.small)
                }
            }
            .heroFrame(frame)
            .overlay(alignment: .topTrailing) { badges }
            .opacity(item.isDeleted ? 0.5 : 1)
            .task(id: loadKey) { thumbnails.load(item) }
            .onAppear { hero.register(item.id, frame) }
            .onDisappear { hero.unregister(item.id, frame) }
    }

    /// Opening the detail from a closed column carries the thumbnail over to
    /// it. Picking a batch, or moving within an open column, only switches.
    private func open() {
        let extending = model.selecting || NSEvent.modifierFlags.contains(.command)
        if !reduceMotion, !detailOpen, !extending, let image = thumbnails.image(for: item) {
            hero.open(item.id, image: image)
        }
        model.click(item)
    }

    private var badges: some View {
        HStack(spacing: 3) {
            if item.sticker.starred {
                badge("star.fill", MemeDockPalette(scheme).star)
            }
            if item.animated {
                badge("play.fill", MemeDockPalette(scheme).secondary)
            }
            if item.isDeleted {
                badge("trash.fill", .secondary)
            }
        }
        .padding(4)
    }

    private func badge(_ symbol: String, _ color: Color) -> some View {
        Image(systemName: symbol)
            .font(.system(size: 9, weight: .bold))
            .foregroundStyle(color)
            .padding(3)
    }

    private var background: AnyShapeStyle {
        let palette = MemeDockPalette(scheme)
        if selected || cursor { return AnyShapeStyle(palette.pressed) }
        if hovering { return AnyShapeStyle(palette.input) }
        return AnyShapeStyle(Color.clear)
    }

    /// Asking again is cheap and is what picks up a thumbnail that was
    /// regenerated for a row that never left the screen.
    private var loadKey: String {
        "\(item.thumbnailStatus)|\(item.thumbnailPath ?? "")"
    }

    @ViewBuilder
    private var menu: some View {
        Button(L10n.text(.copyImage)) { copyImage(false) }
            .disabled(actions.busy)
        Button(L10n.text(.copyFile)) { copyFile() }
            .disabled(actions.busy)
        if item.animated {
            Button(L10n.text(.firstFrame)) { copyImage(true) }
                .disabled(actions.busy)
        }
        Divider()
        Button(L10n.text(.saveAs)) { save() }
        .disabled(actions.busy)
        Button(L10n.text(.share)) { share() }
        .disabled(actions.busy)
        Menu(L10n.text(.exportPreset)) {
            ExportOverridePicker(selection: $actions.override, animated: item.animated)
        }
        .disabled(actions.busy)
        Divider()
        Button(item.sticker.starred ? L10n.text(.unstar) : L10n.text(.star)) {
            let starred = !item.sticker.starred
            Task { await model.setStarred(item, starred) }
        }
        Divider()
        if item.isDeleted {
            Button(L10n.text(.restore)) { Task { await model.restore(item) } }
        } else {
            Button(L10n.text(.deleteStickerTitle), role: .destructive) {
                Task { await model.delete(item) }
            }
        }
    }

    private func copyImage(_ firstFrame: Bool) {
        run { try await model.actions.copyImage(item, firstFrame: firstFrame) }
    }

    private func copyFile() {
        run { try await model.actions.copyFile(item) }
    }

    private func save() {
        run { try await model.actions.save(item) }
    }

    private func share() {
        run { try await model.actions.share(item) }
    }

    private func run(_ body: @escaping () async throws -> Void) {
        Task { await perform(model.status, body) }
    }
}
