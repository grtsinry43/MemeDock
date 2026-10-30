import SwiftUI

enum GroupEditRequest: Identifiable {
    case newCollection, newTag
    case rename(GroupRoute)

    var id: String {
        switch self {
        case .newCollection: "new-collection"
        case .newTag: "new-tag"
        case .rename(let route): "rename-\(route)"
        }
    }

    var title: LocalizedStringKey {
        switch self {
        case .newCollection: "New Collection"
        case .newTag: "New Tag"
        case .rename: "Rename"
        }
    }
}

struct GroupEditorView: View {
    let model: OrganizeModel
    let editing: GroupEditRequest
    var saved: (String) -> Void = { _ in }
    @State private var name = ""
    @FocusState private var focused: Bool
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Name", text: $name).focused($focused)
                        .submitLabel(.done).onSubmit { save() }
                        .disabled(model.isMutating)
                }
                if let error = model.mutationError {
                    Text(error).foregroundStyle(.red)
                }
            }
            .navigationTitle(editing.title).navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }.disabled(model.isMutating)
                }
                ToolbarItem(placement: .confirmationAction) {
                    if model.isMutating {
                        ProgressView()
                    } else {
                        Button("Save", action: save).disabled(
                            name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
            }
            .interactiveDismissDisabled(model.isMutating)
            .task {
                model.dismissMutationError()
                if case .rename(let route) = editing { name = model.name(for: route) ?? "" }
                focused = true
            }
        }
        .presentationDetents([.medium, .large])
    }

    private func save() {
        guard !model.isMutating, !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        Task {
            if await model.save(name: name, editing: editing) {
                saved(name.trimmingCharacters(in: .whitespacesAndNewlines).precomposedStringWithCanonicalMapping)
                dismiss()
            }
        }
    }
}

struct GroupManagementPresentation: ViewModifier {
    let model: OrganizeModel
    @Binding var editing: GroupEditRequest?
    @Binding var deleting: GroupRoute?
    var presentsEditor = true
    var deleted: () -> Void = {}

    func body(content: Content) -> some View {
        content
            .sheet(item: Binding(get: { presentsEditor ? editing : nil }, set: { editing = $0 })) {
                GroupEditorView(model: model, editing: $0)
            }
            .onChange(of: deleting) { _, route in
                guard let route else { return }
                Task {
                    if await model.delete(route) { deleted() }
                    deleting = nil
                }
            }
            .alert(
                "Couldn’t Save Change",
                isPresented: Binding(
                    get: { editing == nil && model.mutationError != nil },
                    set: { if !$0 { model.dismissMutationError() } }
                )
            ) {
                Button("OK") { model.dismissMutationError() }
            } message: {
                Text(model.mutationError ?? "")
            }
    }
}

struct GroupInlineEditor: View {
    let model: OrganizeModel
    let editing: GroupEditRequest
    let finished: () -> Void
    @State private var name = ""
    @FocusState private var focused: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                TextField("Name", text: $name).textFieldStyle(.roundedBorder).focused($focused)
                    .submitLabel(.done).onSubmit(save)
                if model.isMutating {
                    ProgressView()
                } else {
                    Button("Save", systemImage: "checkmark", action: save).labelStyle(.iconOnly)
                        .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Button("Cancel", systemImage: "xmark") { finished() }.labelStyle(.iconOnly)
                }
            }.disabled(model.isMutating)
            if let error = model.mutationError { Text(error).font(.caption).foregroundStyle(.red) }
        }.task(id: editing.id) {
            model.dismissMutationError()
            name = if case .rename(let route) = editing { model.name(for: route) ?? "" } else { "" }
            focused = true
        }
    }
    private func save() {
        guard !model.isMutating, !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        Task { if await model.save(name: name, editing: editing) { finished() } }
    }
}
