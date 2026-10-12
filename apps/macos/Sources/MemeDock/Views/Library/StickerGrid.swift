import AppKit
import MemeDockBridge
import SwiftUI

/// The grid of the current section. Paging belongs to the browser: a row near
/// the end asks for the next page, so scrolling keeps flowing.
struct StickerGrid: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var model: LibraryModel
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var selection: StickerSelection
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var imports: ImportCoordinator

    private static let columns = [GridItem(.adaptive(minimum: 96, maximum: 132), spacing: 12)]

    init(model: LibraryModel) {
        self.model = model
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
        ScrollView {
            LazyVGrid(columns: Self.columns, spacing: 12) {
                ForEach(browser.items) { item in
                    StickerCell(
                        item: item,
                        selected: selection.isSelected(item.id),
                        // While a batch is being picked the ring is nothing but
                        // a second highlight on rows that are already tinted.
                        focused: detail.id == item.id && !model.selecting,
                        model: model,
                        thumbnails: model.thumbnails
                    )
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
            .padding(16)
        }
        .dropDestination(for: URL.self) { urls, _ in
            guard !urls.isEmpty else { return false }
            let collectionId = model.importCollectionId
            Task { await model.imports.importFiles(urls, collectionId: collectionId) }
            return true
        }
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
            } else {
                ProgressView()
                    .controlSize(.small)
            }
            Button(L10n.text(.cancel)) { imports.cancel() }
                .controlSize(.small)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(MemeDockGlass())
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
    let item: StickerItem
    let selected: Bool
    let focused: Bool
    let model: LibraryModel
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
        .overlay {
            RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                .strokeBorder(focused ? MemeDockPalette(scheme).accent.opacity(0.65) : .clear, lineWidth: 1.5)
        }
        .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .onHover { hovering = $0 }
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: selected)
        .onTapGesture { model.click(item) }
        .contextMenu { menu }
        .help(item.sticker.title.isEmpty ? item.sticker.originalName : item.sticker.title)
    }

    private var preview: some View {
        ZStack {
            RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                .fill(MemeDockPalette(scheme).input.opacity(0.45))
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
        .frame(height: 96)
        .overlay(alignment: .topTrailing) { badges }
        .opacity(item.isDeleted ? 0.5 : 1)
        .task(id: loadKey) { thumbnails.load(item) }
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
        if selected { return AnyShapeStyle(palette.accent.opacity(0.12)) }
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
        Button(L10n.text(.copyFile)) { copyFile() }
        if item.animated {
            Button(L10n.text(.firstFrame)) { copyImage(true) }
        }
        Divider()
        Menu(L10n.text(.saveAs)) {
            ForEach(Self.presets, id: \.self) { preset in
                Button(StickerExport.label(preset)) { save(preset) }
            }
        }
        Menu(L10n.text(.share)) {
            ForEach(Self.presets, id: \.self) { preset in
                Button(StickerExport.label(preset)) { share(preset) }
            }
        }
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

    private static let presets: [ExportPreset] = [.original, .compatiblePng, .whiteBackground, .smallJpeg]

    private func copyImage(_ firstFrame: Bool) {
        run { try await model.actions.copyImage(item, firstFrame: firstFrame) }
    }

    private func copyFile() {
        run { try await model.actions.copyFile(item) }
    }

    private func save(_ preset: ExportPreset) {
        run { try await model.actions.save(item, preset: preset) }
    }

    private func share(_ preset: ExportPreset) {
        run { try await model.actions.share(item, preset: preset) }
    }

    private func run(_ body: @escaping () async throws -> Void) {
        Task { await perform(model.status, body) }
    }
}
