import SwiftUI

struct GroupLibraryView: View {
    let session: LibrarySession
    let route: GroupRoute
    let transitionNamespace: Namespace.ID
    let open: (LibraryItem) -> Void
    @State private var browser: LibraryBrowser?

    var body: some View {
        Group {
            if let name = session.organize?.name(for: route) {
                if let browser, let thumbnails = session.thumbnails {
                    groupGrid(browser, thumbnails: thumbnails).navigationTitle(name)
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
                ContentUnavailableView("Group Unavailable", systemImage: "folder.badge.questionmark",
                    description: Text("This group was deleted or replaced. Return to Organize to select it again."))
            }
        }
        .navigationBarTitleDisplayMode(.inline)
        .task {
            if browser == nil {
                browser = session.makeGroupBrowser(for: route)
                await browser?.refresh()
            }
        }
    }

    private func groupGrid(_ browser: LibraryBrowser, thumbnails: ThumbnailStore) -> some View {
        @Bindable var browser = browser
        return LibraryGridView(
            browser: browser, thumbnails: thumbnails, imports: nil, sessionError: nil,
            refresh: { await browser.refresh() }, showProblems: {}, importImages: {},
            transitionNamespace: transitionNamespace, open: open
        )
        .searchable(text: $browser.searchText, prompt: "Search This Group")
    }
}
