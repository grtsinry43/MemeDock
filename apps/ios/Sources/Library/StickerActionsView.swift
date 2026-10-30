import MemeDockBridge
import SwiftUI

struct StickerActionsToolbar: ViewModifier {
    let browser: LibraryBrowser
    let organize: OrganizeModel

    func body(content: Content) -> some View {
        @Bindable var browser = browser
        return
            content
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button(
                        browser.isSelecting ? "Done" : "Select",
                        systemImage: browser.isSelecting ? "xmark" : "checkmark.circle"
                    ) {
                        if browser.isSelecting { browser.endSelection() } else { browser.startSelection() }
                    }.labelStyle(.iconOnly)
                        .disabled(browser.isMutating || (!browser.isSelecting && browser.items.isEmpty))
                }
                if browser.isSelecting {
                    ToolbarItemGroup(placement: .bottomBar) {
                        if browser.isMutating {
                            ProgressView(
                                value: Double(browser.actionCompleted), total: Double(max(1, browser.actionTotal))
                            )
                            .frame(maxWidth: 100)
                            Button("Cancel", systemImage: "xmark") { browser.cancelAction() }.labelStyle(.iconOnly)
                        } else {
                            Text("\(browser.selected.count) selected").font(.subheadline)
                            Spacer()
                            Menu {
                                Section {
                                    Button("Organize", systemImage: "tag") { browser.isPresentingActions = true }
                                    Button("Favorite", systemImage: "star") {
                                        Task { _ = await browser.apply(.star(value: true)) }
                                    }
                                    Button("Remove Favorites", systemImage: "star.slash") {
                                        Task { _ = await browser.apply(.star(value: false)) }
                                    }
                                }
                                Section {
                                    Button("Move to Trash", systemImage: "trash", role: .destructive) {
                                        Task { _ = await browser.apply(.delete) }
                                    }
                                }
                            } label: {
                                Label("Actions", systemImage: "ellipsis.circle")
                            }
                            .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                            .disabled(browser.selected.isEmpty)
                        }
                    }
                }
            }
            .sheet(isPresented: $browser.isPresentingActions) {
                StickerActionsView(browser: browser, organize: organize)
            }
            .alert(
                "Some Changes Couldn’t Be Saved",
                isPresented: Binding(
                    get: { !browser.isPresentingActions && browser.actionError != nil },
                    set: { if !$0 { browser.dismissActionError() } }
                )
            ) {
                Button("OK") { browser.dismissActionError() }
            } message: {
                Text(browser.actionError ?? "")
            }
    }
}

struct StickerActionsView: View {
    let browser: LibraryBrowser
    let organize: OrganizeModel
    @State private var tags: Set<String> = []
    @State private var removeTags = false
    @State private var editing: GroupEditRequest?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("\(browser.selected.count) selected").foregroundStyle(.secondary)
                    if browser.isMutating {
                        ProgressView(value: Double(browser.actionCompleted), total: Double(max(1, browser.actionTotal)))
                        Text("Processed \(browser.actionCompleted) of \(browser.actionTotal)").font(.caption)
                    }
                    if let error = browser.actionError { Text(error).foregroundStyle(.red) }
                }
                Section("Collection") {
                    ForEach(organize.collections, id: \.collection.id) { summary in
                        Button {
                            apply(
                                .assign(
                                    collection: EntityReference(
                                        id: summary.collection.id, generation: summary.collection.lifecycle.generation))
                            )
                        } label: {
                            Label(summary.collection.name, systemImage: "folder")
                        }
                    }
                    Button("Remove from Collection", systemImage: "folder.badge.minus") {
                        let expected: EntityReference?
                        if case .collection(let id) = browser.filter,
                            let value = organize.collections.first(where: { $0.collection.id == id })?.collection
                        {
                            expected = EntityReference(id: id, generation: value.lifecycle.generation)
                        } else {
                            expected = nil
                        }
                        apply(.clearCollection(expected: expected))
                    }
                    Button("New Collection", systemImage: "folder.badge.plus") { editing = .newCollection }
                }
                .disabled(browser.isMutating)
                Section {
                    Button("Favorite", systemImage: "star") { apply(.star(value: true)) }
                    Button("Remove Favorites", systemImage: "star.slash") { apply(.star(value: false)) }
                    Button("Move to Trash", systemImage: "trash", role: .destructive) { apply(.delete) }
                } footer: {
                    Text("Original images are kept. You can restore these stickers from Trash.")
                }
                .disabled(browser.isMutating)
                Section("Tags") {
                    Picker("Operation", selection: $removeTags) {
                        Text("Add Tags").tag(false)
                        Text("Remove Tags").tag(true)
                    }.pickerStyle(.segmented)
                    ForEach(organize.tags, id: \.id) { tag in
                        Toggle(
                            tag.name,
                            isOn: Binding(
                                get: { tags.contains(tag.id) },
                                set: { if $0 { tags.insert(tag.id) } else { tags.remove(tag.id) } }
                            ))
                    }
                    Button(removeTags ? "Remove Selected Tags" : "Add Selected Tags") {
                        let references = organize.tags.filter { tags.contains($0.id) }.map {
                            EntityReference(id: $0.id, generation: $0.lifecycle.generation)
                        }
                        apply(removeTags ? .removeTags(tags: references) : .addTags(tags: references))
                    }.disabled(tags.isEmpty)
                    Button("New Tag", systemImage: "plus") { editing = .newTag }
                }
                .disabled(browser.isMutating)
            }
            .navigationTitle("Organize Stickers").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }.disabled(browser.isMutating)
                }
                ToolbarItem(placement: .confirmationAction) {
                    if browser.isMutating { Button("Stop") { browser.cancelAction() } }
                }
            }
            .interactiveDismissDisabled(browser.isMutating || organize.isMutating)
            .sheet(item: $editing) { request in
                GroupEditorView(model: organize, editing: request) { name in
                    if case .newTag = request, let id = organize.tags.first(where: { $0.name == name })?.id {
                        tags.insert(id)
                    }
                }
            }
        }
    }

    private func apply(_ action: BatchAction) {
        Task {
            if await browser.apply(action) {
                browser.endSelection()
                dismiss()
            }
        }
    }
}
