import SwiftUI

struct GroupLibraryView: View {
    let session: LibrarySession
    let route: GroupRoute
    let hero: StickerHero
    let open: (LibraryItem, CGImage?) -> Void
    @State private var browser: LibraryBrowser?
    @State private var editing: GroupEditRequest?
    @State private var deleting: GroupRoute?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Group {
            if let name = session.organize?.name(for: route) {
                if let browser, let thumbnails = session.thumbnails, let organize = session.organize {
                    VStack(spacing: 0) {
                        if let editing {
                            GroupInlineEditor(model: organize, editing: editing) { self.editing = nil }.padding(16)
                        }
                        groupGrid(browser, thumbnails: thumbnails, organize: organize)
                    }.navigationTitle(name)
                } else {
                    ProgressView().navigationTitle(name)
                }
            } else if session.organize?.isLoading == true {
                ProgressView()
            } else if let model = session.organize, model.error != nil {
                ContentUnavailableView {
                    Label("Couldn’t Load Group", systemImage: "exclamationmark.triangle")
                } actions: {
                    Button("Retry") { Task { await model.refresh() } }
                }
            } else {
                ContentUnavailableView(
                    "Group Unavailable", systemImage: "folder.badge.questionmark",
                    description: Text("This group was deleted or replaced. Return to Organize to select it again."))
            }
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
        .toolbar {
            if let model = session.organize, model.name(for: route) != nil {
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        Button("Rename", systemImage: "pencil") { editing = .rename(route) }
                        if case .collection = route.filter {
                            NavigationLink("Reorder Stickers", value: OrganizeDestination.reorderItems(route))
                        }
                        Button("Move to Trash", systemImage: "trash", role: .destructive) { deleting = route }
                    } label: {
                        Label("Group Actions", systemImage: "ellipsis")
                    }
                    .disabled(model.isMutating)
                }
            }
        }
        .confirmationDialog(
            "Move to Trash?",
            isPresented: Binding(
                get: { deleting != nil }, set: { if !$0 { deleting = nil } }
            ), titleVisibility: .visible, presenting: deleting
        ) { target in
            Button("Move to Trash", role: .destructive) {
                Task {
                    _ = await session.organize?.delete(target)
                }
            }
        } message: { _ in
            Text("The group will be moved to Trash. Original images are kept.")
        }
        .alert(
            "Couldn’t Save Change",
            isPresented: Binding(
                get: { editing == nil && session.organize?.mutationError != nil },
                set: { if !$0 { session.organize?.dismissMutationError() } }
            )
        ) {
            Button("OK") { session.organize?.dismissMutationError() }
        } message: {
            Text(session.organize?.mutationError ?? "")
        }
        .onChange(of: session.organize?.name(for: route)) { previous, current in
            if previous != nil, current == nil, session.phase == .ready, !session.isRestoring {
                session.feedback.post(String(localized: "Moved to Trash"), action: .viewTrash)
                dismiss()
            }
        }
        .task(id: session.organize?.name(for: route)) {
            if browser == nil {
                browser = session.makeGroupBrowser(for: route)
                await browser?.refresh()
            }
        }
    }

    private func groupGrid(_ browser: LibraryBrowser, thumbnails: ThumbnailStore, organize: OrganizeModel) -> some View
    {
        @Bindable var browser = browser
        return LibraryGridView(
            browser: browser, thumbnails: thumbnails, imports: nil, session: session, sessionError: nil,
            refresh: { await browser.refresh() }, showProblems: {}, importImages: {},
            hero: hero, open: open
        )
        .modifier(StickerActionsToolbar(browser: browser, organize: organize))
    }
}
