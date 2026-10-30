import SwiftUI

@main
struct MemeDockApp: App {
    @State private var session: LibrarySession

    init() {
        let session = LibrarySession()
        _session = State(initialValue: session)
        BackgroundSyncScheduler.register(session: session)
    }

    var body: some Scene {
        WindowGroup {
            RootView(session: session)
                .task { await session.start() }
        }
    }
}
