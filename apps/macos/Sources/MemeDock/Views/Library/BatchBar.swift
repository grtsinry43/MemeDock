import MemeDockBridge
import SwiftUI

/// The batch bar shows up while several stickers are being picked. Every action
/// goes through one core batch call, so a mixed selection is decided together
/// and the per-sticker outcome is reported once.
struct BatchBar: View {
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
        HStack(spacing: 10) {
            if selection.running {
                ProgressView()
                    .controlSize(.small)
                Text(L10n.text(.batchRunning, selection.count))
                    .foregroundStyle(.secondary)
            } else {
                Text(L10n.text(.selectedCount, selection.count))
                    .monospacedDigit()
            }
            Spacer(minLength: 8)
            actions
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
        .overlay(alignment: .bottom) { Divider() }
    }

    @ViewBuilder
    private var actions: some View {
        Button(L10n.text(.selectAll)) { model.selectAll() }
            .disabled(selection.running || browser.items.isEmpty)
        Button(L10n.text(.clearSelection)) { model.clearSelection() }
            .disabled(selection.running || selection.count == 0)
        Divider().frame(height: 16)
        Menu(L10n.text(.assignCollection)) {
            ForEach(collections.active, id: \.id) { collection in
                Button(collection.name) {
                    run(.assign(collection: reference(collection)))
                }
            }
            Divider()
            Button(L10n.text(.clearCollection)) { run(.clearCollection(expected: nil)) }
        }
        .disabled(busy)
        Menu(L10n.text(.addTags)) {
            ForEach(tags.active, id: \.id) { tag in
                Button(tag.name) { run(.addTags(tags: [reference(tag)])) }
            }
        }
        .disabled(busy || tags.active.isEmpty)
        Menu(L10n.text(.removeTags)) {
            ForEach(tags.active, id: \.id) { tag in
                Button(tag.name) { run(.removeTags(tags: [reference(tag)])) }
            }
        }
        .disabled(busy || tags.active.isEmpty)
        Button(L10n.text(.star)) { run(.star(value: true)) }
            .disabled(busy)
        Button(L10n.text(.unstar)) { run(.star(value: false)) }
            .disabled(busy)
        if trashed {
            Button(L10n.text(.restore)) { run(.restore) }
                .disabled(busy)
        } else {
            Button(L10n.text(.deleteStickerTitle), role: .destructive) { run(.delete) }
                .disabled(busy)
        }
    }

    /// A selection in the trash can only be restored, and one outside it can
    /// only be deleted: the section decides which of the two is offered.
    private var trashed: Bool {
        browser.filter == .deleted
    }

    private var busy: Bool {
        selection.running || selection.count == 0
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
