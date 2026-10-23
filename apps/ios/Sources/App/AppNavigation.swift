import Foundation
import Observation

enum AppTab: Hashable { case stickers, organize, mine }

struct GroupRoute: Hashable {
    let filter: LibraryFilter
    let generation: Int64
}

enum OrganizeDestination: Hashable {
    case group(GroupRoute)
    case sticker(StickerDetailRoute)
}

enum MineDestination: Hashable { case libraryInfo }

@MainActor
@Observable
final class AppNavigation {
    var selected: AppTab = .stickers
    var homePath: [StickerDetailRoute] = []
    var organizePath: [OrganizeDestination] = []
    var minePath: [MineDestination] = []

    func showImportProgress() {
        homePath = []
        selected = .stickers
    }

    func showImported(_ id: String, browser: LibraryBrowser) async {
        showImportProgress()
        await browser.showImported(id)
    }
}
