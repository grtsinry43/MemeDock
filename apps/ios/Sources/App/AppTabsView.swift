import SwiftUI

struct AppTabsView: View {
    @Bindable var navigation: AppNavigation
    let session: LibrarySession
    @Namespace private var organizeNamespace
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TabView(selection: $navigation.selected) {
            Tab("Stickers", systemImage: "face.smiling", value: AppTab.stickers) {
                LibraryHomeView(navigation: navigation, session: session)
            }
            Tab("Organize", systemImage: "folder", value: AppTab.organize) {
                NavigationStack(path: $navigation.organizePath) {
                    if let organize = session.organize {
                        OrganizeView(model: organize)
                        .navigationDestination(for: OrganizeDestination.self) { destination in
                            switch destination {
                            case .group(let route):
                                GroupLibraryView(
                                    session: session, route: route, transitionNamespace: organizeNamespace
                                ) { item in
                                    navigation.organizePath.append(.sticker(StickerDetailRoute(item)))
                                }
                            case .sticker(let route):
                                if reduceMotion {
                                    detail(route)
                                } else {
                                    detail(route).navigationTransition(.zoom(sourceID: route, in: organizeNamespace))
                                }
                            }
                        }
                    }
                }
            }
            Tab("Mine", systemImage: "person.crop.circle", value: AppTab.mine) {
                NavigationStack(path: $navigation.minePath) {
                    MineView(session: session)
                        .navigationDestination(for: MineDestination.self) { _ in
                            LibraryInfoView(session: session)
                        }
                }
            }
        }
        .tabViewStyle(.sidebarAdaptable)
        .safeAreaInset(edge: .bottom) {
            if navigation.selected != .stickers, let imports = session.imports, !imports.items.isEmpty {
                HStack {
                    if imports.isRunning { ProgressView() }
                    Text(imports.isRunning ? "Import in Progress" : "Import Finished")
                        .font(.subheadline)
                    Spacer()
                    if !imports.isRunning, let id = imports.items.last(where: {
                        $0.status == .created || $0.status == .reused
                    })?.stickerID, !imports.hasProblems, let browser = session.browser {
                        Button("View Imported") {
                            Task { await navigation.showImported(id, browser: browser) }
                        }
                    } else {
                        Button("View Import") { navigation.showImportProgress() }
                    }
                }
                .padding(.horizontal, 16).padding(.vertical, 10)
                .background(.bar)
            }
        }
    }

    private func detail(_ route: StickerDetailRoute) -> some View {
        StickerDetailView(session: session, route: route, isVisible: navigation.selected == .organize)
    }
}
