import Foundation
import SwiftUI

/// Makes a view a drag source for one sticker.
///
/// The provider is built when the drag starts, after the export has finished,
/// so targets get real data instead of a promise.
extension View {
    func stickerDragSource(_ actions: StickerActions, item: StickerItem) -> some View {
        modifier(StickerDragModifier(actions: actions, item: item))
    }
}

private struct StickerDragModifier: ViewModifier {
    @ObservedObject var actions: StickerActions
    let item: StickerItem
    @State private var hovering = false

    func body(content: Content) -> some View {
        content.onHover { inside in
            hovering = inside
            if inside { actions.prepareDrag(for: item) }
            else { actions.cancelDragPreparation(for: item) }
        }
        .onChange(of: actions.dragPreparationKey(for: item)) { _, _ in
            if hovering { actions.prepareDrag(for: item) }
        }
        .onDisappear { actions.cancelDragPreparation(for: item) }
        .onDrag {
            var provider = NSItemProvider()
            MainActor.assumeIsolated { provider = actions.dragProvider(for: item) }
            return provider
        }
    }
}
