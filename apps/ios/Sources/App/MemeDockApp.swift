import SwiftUI

@main
struct MemeDockApp: App {
    @State private var session = LibrarySession()

    var body: some Scene {
        WindowGroup {
            RootView(session: session)
                .task { await session.start() }
        }
    }
}
