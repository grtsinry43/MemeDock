import SwiftUI

struct AppTabsView: View {
    @Bindable var navigation: AppNavigation
    let session: LibrarySession

    var body: some View {
        TabView(selection: $navigation.selected) {
            Tab("Stickers", systemImage: "face.smiling", value: AppTab.stickers) {
                LibraryHomeView(navigation: navigation, session: session)
            }
            Tab("Organize", systemImage: "folder", value: AppTab.organize) {
                NavigationStack(path: $navigation.organizePath) {
                    if let organize = session.organize, let thumbnails = session.thumbnails {
                        OrganizeView(model: organize, thumbnails: thumbnails)
                            .navigationDestination(for: OrganizeDestination.self) { destination in
                                switch destination {
                                case .collections:
                                    CollectionsView(model: organize, thumbnails: thumbnails)
                                case .reorderCollections:
                                    CollectionsView(model: organize, thumbnails: thumbnails, reordering: true)
                                case .reorderItems(let route):
                                    CollectionItemsOrderView(session: session, route: route)
                                case .group(let route):
                                    GroupLibraryView(session: session, route: route, hero: navigation.hero) {
                                        item, image in
                                        navigation.hero.present(StickerDetailRoute(item), image: image)
                                    }
                                }
                            }
                    }
                }
            }
            Tab("Mine", systemImage: "person.crop.circle", value: AppTab.mine) {
                NavigationStack(path: $navigation.minePath) {
                    MineView(session: session)
                        .navigationDestination(for: MineDestination.self) { destination in
                            Group {
                                switch destination {
                                case .libraryInfo: LibraryInfoView(session: session)
                                case .trash:
                                    if let trash = session.trash, let thumbnails = session.thumbnails {
                                        TrashView(model: trash, thumbnails: thumbnails, hero: navigation.hero) {
                                            route, image in navigation.hero.present(route, image: image)
                                        }
                                    }
                                case .backup:
                                    if let backup = session.backup { BackupView(model: backup) }
                                case .sync:
                                    if let sync = session.sync { SyncView(model: sync) }
                                case .licenses:
                                    LicensesView()
                                }
                            }
                            .toolbar(.hidden, for: .tabBar)
                        }
                }
            }
        }
        .tabViewStyle(.sidebarAdaptable)
    }
}
