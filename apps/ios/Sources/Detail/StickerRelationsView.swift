import MemeDockBridge
import SwiftUI

struct StickerRelationsView: View {
    let model: StickerDetailModel
    let catalog: OrganizeModel
    let detail: StickerDetail
    @State private var collection: String?
    @State private var tags: Set<String> = []
    @State private var initialized = false
    @State private var editing: GroupEditRequest?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Collection") {
                    Picker("Collection", selection: $collection) {
                        Text("No Collection").tag(nil as String?)
                        ForEach(catalog.collections, id: \.collection.id) { item in
                            Text(item.collection.name).tag(Optional(item.collection.id))
                        }
                    }.pickerStyle(.inline)
                    Button("New Collection", systemImage: "folder.badge.plus") { editing = .newCollection }
                }
                Section("Tags") {
                    ForEach(catalog.tags, id: \.id) { tag in
                        Toggle(
                            tag.name,
                            isOn: Binding(
                                get: { tags.contains(tag.id) },
                                set: { if $0 { tags.insert(tag.id) } else { tags.remove(tag.id) } }))
                    }
                    Button("New Tag", systemImage: "plus") { editing = .newTag }
                }
                if let error = model.managementError ?? catalog.error { Text(error).foregroundStyle(.red) }
            }
            .disabled(model.isManaging || catalog.isMutating)
            .navigationTitle("Organize").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }.disabled(model.isManaging)
                }
                ToolbarItem(placement: .confirmationAction) {
                    if model.isManaging { ProgressView() } else { Button("Done") { save() } }
                }
            }
            .interactiveDismissDisabled(model.isManaging || catalog.isMutating || changed)
            .task {
                if !initialized {
                    collection = detail.collection?.id
                    tags = Set(detail.tags.map(\.id))
                    initialized = true
                }
                await catalog.refresh()
            }
            .sheet(item: $editing) { request in
                GroupEditorView(model: catalog, editing: request) { name in
                    switch request {
                    case .newCollection:
                        collection = catalog.collections.first { $0.collection.name == name }?.collection.id
                    case .newTag: if let id = catalog.tags.first(where: { $0.name == name })?.id { tags.insert(id) }
                    case .rename: break
                    }
                }
            }
        }
    }
    private var changed: Bool { collection != detail.collection?.id || tags != Set(detail.tags.map(\.id)) }
    private func save() {
        let chosen = catalog.collections.first { $0.collection.id == collection }?.collection
        let target = chosen.map { EntityReference(id: $0.id, generation: $0.lifecycle.generation) }
        let labels = catalog.tags.filter { tags.contains($0.id) }.map {
            EntityReference(id: $0.id, generation: $0.lifecycle.generation)
        }
        Task { if await model.organize(collection: target, tags: labels) { dismiss() } }
    }
}
