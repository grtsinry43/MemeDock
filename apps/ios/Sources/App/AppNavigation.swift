import Foundation
import Observation

enum AppTab: Hashable { case stickers, organize, mine }

struct GroupRoute: Hashable {
    let filter: LibraryFilter
    let generation: Int64
}

enum OrganizeDestination: Hashable {
    case collections
    case reorderCollections
    case reorderItems(GroupRoute)
    case group(GroupRoute)
}

enum MineDestination: Hashable {
    case libraryInfo, trash, backup, sync, licenses
}

@MainActor
@Observable
final class AppNavigation {
    var selected: AppTab = .stickers
    var organizePath: [OrganizeDestination] = []
    var minePath: [MineDestination] = []
    /// Sticker details open above every tab rather than inside a tab's stack.
    let hero = StickerHero()

    func showImportProgress() {
        hero.reset()
        selected = .stickers
    }

    func showImported(_ id: String, browser: LibraryBrowser) async {
        showImportProgress()
        await browser.showImported(id)
    }
}
