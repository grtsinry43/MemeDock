import MemeDockBridge
import SwiftUI

/// The right column while stickers are being picked. Choosing many and
/// inspecting one are different jobs, so this replaces the detail pane instead
/// of sitting next to it: an edit here is sent once, for the whole selection,
/// through a single batch call, and never for the one sticker that happened to
/// be open.
struct BatchPane: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var model: LibraryModel
    @ObservedObject private var selection: StickerSelection
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var collections: CollectionCatalog
    @ObservedObject private var tags: TagCatalog

    init(model: LibraryModel) {
        self.model = model
        _selection = ObservedObject(wrappedValue: model.selection)
        _browser = ObservedObject(wrappedValue: model.browser)
        _collections = ObservedObject(wrappedValue: model.collections)
        _tags = ObservedObject(wrappedValue: model.tags)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: MemeDockGeometry.sectionGap) {
                    countSection
                    Divider()
                    if trashed {
                        restoreButton
                    } else {
                        collectionRow
                        tagRows
                        Divider()
                        starButtons
                        Divider()
                        deleteButton
                    }
                }
                .padding(16)
            }
        }
        .font(MemeDockTypography.body)
        .background(MemeDockPalette(scheme).raised)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text(L10n.text(.batchOrganize))
                .font(MemeDockTypography.rowTitle)
            Spacer()
            Button {
                model.setSelecting(false)
            } label: {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
            .help(L10n.text(.done))
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(MemeDockGlass())
    }

    /// The count line carries what the run is allowed to cover: the core refuses
    /// more than one call can hold, and a select-all only reaches the pages the
    /// grid has streamed in. Both are stated here, so neither arrives as a
    /// failure after the fact.
    private var countSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            if selection.running {
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text(L10n.text(.batchRunning, selection.count))
                        .font(MemeDockTypography.body)
                        .foregroundStyle(.secondary)
                }
            } else {
                Text(L10n.text(.selectedCount, selection.count))
                    .font(MemeDockTypography.body)
                    .monospacedDigit()
                if selection.count == 0 {
                    Text(L10n.text(.selectionHint))
                        .font(MemeDockTypography.body)
                        .foregroundStyle(.secondary)
                } else if overLimit {
                    Text(L10n.text(.selectionOverLimit, selection.count, StickerSelection.limit))
                        .font(MemeDockTypography.body)
                        .foregroundStyle(.orange)
                } else if browser.hasMore {
                    Text(L10n.text(.selectionLoadedOnly))
                        .font(MemeDockTypography.body)
                        .foregroundStyle(.secondary)
                }
            }
            HStack(spacing: 8) {
                Button(L10n.text(.selectAll)) { model.selectAll() }
                    .disabled(selection.running || browser.items.isEmpty)
                Button(L10n.text(.clearSelection)) { model.clearSelection() }
                    .disabled(selection.running || selection.count == 0)
            }
        }
    }

    private var collectionRow: some View {
        row(L10n.text(.collections)) {
            Menu(L10n.text(.assignCollection)) {
                ForEach(collections.active, id: \.id) { collection in
                    Button(collection.name) {
                        run(.assign(collection: reference(collection)))
                    }
                }
                Divider()
                Button(L10n.text(.clearCollection)) { run(.clearCollection(expected: nil)) }
            }
            .frame(maxWidth: 170, alignment: .trailing)
            .disabled(busy || collections.active.isEmpty)
        }
    }

    private var tagRows: some View {
        row(L10n.text(.tags)) {
            VStack(alignment: .trailing, spacing: 6) {
                Menu(L10n.text(.addTags)) {
                    ForEach(tags.active, id: \.id) { tag in
                        Button(tag.name) { run(.addTags(tags: [reference(tag)])) }
                    }
                }
                Menu(L10n.text(.removeTags)) {
                    ForEach(tags.active, id: \.id) { tag in
                        Button(tag.name) { run(.removeTags(tags: [reference(tag)])) }
                    }
                }
            }
            .frame(maxWidth: 170, alignment: .trailing)
            .disabled(busy || tags.active.isEmpty)
        }
    }

    private var starButtons: some View {
        HStack(spacing: 8) {
            Button(L10n.text(.star)) { run(.star(value: true)) }
                .disabled(busy)
            Button(L10n.text(.unstar)) { run(.star(value: false)) }
                .disabled(busy)
        }
    }

    private var deleteButton: some View {
        Button(L10n.text(.deleteStickerTitle), role: .destructive) { run(.delete) }
            .disabled(busy)
    }

    /// A selection in the trash can only be restored, which is the same rule the
    /// detail pane follows for a single sticker.
    private var restoreButton: some View {
        Button(L10n.text(.restore)) { run(.restore) }
            .disabled(busy)
    }

    private var trashed: Bool {
        browser.filter == .deleted
    }

    /// A call that cannot run disables every row: nothing is picked yet, a call
    /// is already in flight, or the selection is past what one call can carry.
    private var busy: Bool {
        selection.running || selection.count == 0 || overLimit
    }

    private var overLimit: Bool {
        selection.count > StickerSelection.limit
    }

    private func row<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        HStack(spacing: 8) {
            Text(title)
                .foregroundStyle(.secondary)
            Spacer(minLength: 8)
            content()
        }
    }

    private func reference(_ collection: CollectionMetadata) -> EntityReference {
        EntityReference(id: collection.id, generation: collection.lifecycle.generation)
    }

    private func reference(_ tag: TagMetadata) -> EntityReference {
        EntityReference(id: tag.id, generation: tag.lifecycle.generation)
    }

    private func run(_ action: BatchAction) {
        Task { await selection.run(action) }
    }
}
