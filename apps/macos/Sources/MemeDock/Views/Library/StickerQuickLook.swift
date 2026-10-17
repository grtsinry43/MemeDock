import AppKit
import QuickLookUI

/// One sticker offered to the system Quick Look panel. The panel would
/// otherwise show the library's internal file name.
final class StickerPreviewItem: NSObject, QLPreviewItem {
    let previewItemURL: URL?
    let previewItemTitle: String?

    init(url: URL, title: String) {
        self.previewItemURL = url
        self.previewItemTitle = title
    }
}
