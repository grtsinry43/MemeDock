import SwiftUI

struct RootView: View {
    let session: LibrarySession
    @State private var navigation = AppNavigation()

    var body: some View {
        Group {
            switch session.phase {
            case .ready:
                AppTabsView(navigation: navigation, session: session)
            case .idle, .opening, .closing:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                ContentUnavailableView {
                    Label("Library Unavailable", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(message)
                } actions: {
                    Button("Retry") { Task { await session.retry() } }
                }
            case .closed:
                ContentUnavailableView("Library Closed", systemImage: "externaldrive")
            }
        }
        .onChange(of: session.phase) { _, phase in
            if phase != .ready { navigation = AppNavigation() }
        }
    }
}
