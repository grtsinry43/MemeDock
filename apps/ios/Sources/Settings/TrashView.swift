import MemeDockBridge
import SwiftUI

struct TrashView: View {
    let model: TrashModel
    let thumbnails: ThumbnailStore
    let hero: StickerHero
    let open: (StickerDetailRoute, CGImage?) -> Void
    @State private var selection: Set<String> = []
    @State private var editMode: EditMode = .inactive
    @State private var purgeTargets: [TrashTarget] = []
    @State private var showPurge = false

    var body: some View {
        List(selection: $selection) {
            if model.isMutating {
                Section {
                    ProgressView(value: Double(model.completed), total: Double(max(1, model.total)))
                    Text("Processed \(model.completed) of \(model.total)")
                    Button("Stop") { model.stop() }
                }
            }
            if let error = model.error {
                Section {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") { Task { await model.load() } }.disabled(model.isMutating)
                }
            }
            if !model.rows.isEmpty {
                Section("Stickers") {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 4)], spacing: 4) {
                        ForEach(model.rows.filter { $0.item != nil }) { row in
                            if let item = row.item {
                                StickerThumbnailView(item: item, store: thumbnails, hero: hero) { image in
                                    if editMode.isEditing {
                                        if !selection.insert(row.id).inserted { selection.remove(row.id) }
                                    } else {
                                        open(StickerDetailRoute(item, includesDeleted: true), image)
                                    }
                                }
                                .overlay(alignment: .topTrailing) {
                                    if editMode.isEditing {
                                        Image(
                                            systemName: selection.contains(row.id) ? "checkmark.circle.fill" : "circle"
                                        )
                                        .font(.title2).foregroundStyle(Color.accentColor).padding(6).allowsHitTesting(
                                            false)
                                    }
                                }
                                .contextMenu {
                                    Button("Restore") {
                                        Task { await model.apply(to: [row.target], permanently: false) }
                                    }
                                    Button("Permanently Delete", role: .destructive) {
                                        purgeTargets = [row.target]
                                        showPurge = true
                                    }
                                }
                            }
                        }
                    }.listRowInsets(EdgeInsets(top: 8, leading: 0, bottom: 8, trailing: 0)).listRowBackground(
                        Color.clear)
                }
                section("Collections", kind: .collection, icon: "folder")
                section("Tags", kind: .tag, icon: "tag")
            } else if !model.isLoading && !model.isMutating {
                ContentUnavailableView(
                    "Trash Is Empty", systemImage: "trash",
                    description: Text(
                        "Deleted stickers, collections, and tags stay here until you restore or permanently delete them."
                    ))
            }
            if model.isLoading { ProgressView() }
            if model.hasMore {
                Button("Load More") { Task { await model.load(more: true) } }.disabled(model.isMutating)
            }
        }
        .environment(\.editMode, $editMode)
        .navigationTitle("Trash")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(
                    editMode.isEditing ? "Done" : "Select",
                    systemImage: editMode.isEditing ? "xmark" : "checkmark.circle"
                ) {
                    editMode = editMode.isEditing ? .inactive : .active
                    selection = []
                }.labelStyle(.iconOnly).disabled(model.isMutating || model.rows.isEmpty)
            }
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    Button("Retry File Cleanup", systemImage: "arrow.clockwise") { Task { await model.retryCleanup() } }
                    Button("Empty Trash", systemImage: "trash", role: .destructive) {
                        purgeTargets = model.allTargets
                        showPurge = true
                    }
                } label: {
                    Label("More Actions", systemImage: "ellipsis")
                }
                .disabled(model.isMutating || model.allTargets.isEmpty)
            }
            if editMode.isEditing {
                ToolbarItemGroup(placement: .bottomBar) {
                    Text("\(selection.count) selected").font(.subheadline)
                    Spacer()
                    Button("Restore") {
                        let targets = model.rows.filter { selection.contains($0.id) }.map(\.target)
                        Task {
                            await model.apply(to: targets, permanently: false)
                            retainRemainingSelection()
                        }
                    }.disabled(selection.isEmpty || model.isMutating)
                    Button("Permanently Delete", role: .destructive) {
                        purgeTargets = model.rows.filter { selection.contains($0.id) }.map(\.target)
                        showPurge = true
                    }.disabled(selection.isEmpty || model.isMutating)
                }
            }
        }
        .confirmationDialog("Permanently Delete?", isPresented: $showPurge, titleVisibility: .visible) {
            Button("Permanently Delete", role: .destructive) {
                let confirmed = purgeTargets
                Task {
                    await model.apply(to: confirmed, permanently: true)
                    retainRemainingSelection()
                }
            }
        } message: {
            Text(
                "Permanently delete \(purgeTargets.count) items? This cannot be undone. Items added to Trash later will not be deleted."
            )
        }
        .task { await model.load() }
        .refreshable { await model.load() }
    }

    private func retainRemainingSelection() {
        selection.formIntersection(Set(model.allTargets.map { "\($0.kind)-\($0.target.id)-\($0.target.generation)" }))
    }

    @ViewBuilder
    private func section(_ title: LocalizedStringKey, kind: TrashEntityKind, icon: String) -> some View {
        let rows = model.rows.filter { $0.target.kind == kind }
        if !rows.isEmpty {
            Section {
                ForEach(rows) { row in
                    Label(row.title, systemImage: icon)
                        .tag(row.id)
                        .contextMenu {
                            Button("Restore", systemImage: "arrow.uturn.backward") {
                                Task { await model.apply(to: [row.target], permanently: false) }
                            }
                            Button("Permanently Delete", systemImage: "trash", role: .destructive) {
                                purgeTargets = [row.target]
                                showPurge = true
                            }
                        }
                        .swipeActions(allowsFullSwipe: false) {
                            Button("Restore") { Task { await model.apply(to: [row.target], permanently: false) } }.tint(
                                .blue)
                            Button("Permanently Delete", role: .destructive) {
                                purgeTargets = [row.target]
                                showPurge = true
                            }
                        }
                        .disabled(model.isMutating)
                }
            } header: {
                Text(title)
            }
        }
    }
}
