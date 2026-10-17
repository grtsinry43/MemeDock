import Foundation
import SwiftUI

/// Makes a view a drag source for one sticker.
///
/// The provider is built when the drag starts, after the export has finished,
/// so targets get real data instead of a promise.
extension View {
    func stickerDragSource(_ actions: StickerActions, item: StickerItem) -> some View {
        onDrag {
            var provider = NSItemProvider()
            MainActor.assumeIsolated { provider = actions.dragProvider(for: item) }
            return provider
        }
    }
}
