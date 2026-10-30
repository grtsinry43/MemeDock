import SwiftUI

struct StickerPreviewView: View {
    let model: StickerDetailModel
    var transition = DetailTransition()
    let retry: () -> Void
    @Environment(\.displayScale) private var displayScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var scale: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var dismissing = false
    @GestureState private var magnification: CGFloat = 1
    @GestureState private var translation: CGSize = .zero

    var body: some View {
        VStack(spacing: 12) {
            GeometryReader { geometry in
                ZStack {
                    if let image = shown {
                        Image(decorative: image, scale: displayScale).resizable().scaledToFit()
                            .animation(nil) { $0.opacity(transition.previewHidden ? 0 : 1) }
                    } else if model.phase == .loading || model.playback.state == .loading {
                        ProgressView()
                    } else {
                        Image(systemName: "photo").font(.largeTitle).foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .scaleEffect(min(4, max(1, scale * magnification)))
                .offset(
                    clamped(
                        CGSize(width: offset.width + translation.width, height: offset.height + translation.height),
                        scale: min(4, max(1, scale * magnification)), size: geometry.size)
                )
                .frame(width: geometry.size.width, height: geometry.size.height).clipped()
                .contentShape(.rect)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { transition.previewFrame($0) }
                .gesture(
                    MagnifyGesture().updating($magnification) { value, state, _ in state = value.magnification }
                        .onEnded { value in
                            scale = min(4, max(1, scale * value.magnification))
                            offset = clamped(offset, scale: scale, size: geometry.size)
                        }
                )
                .simultaneousGesture(
                    DragGesture().updating($translation) { value, state, _ in
                        if scale > 1 { state = value.translation }
                    }.onEnded { value in
                        if scale > 1 {
                            offset = clamped(
                                CGSize(
                                    width: offset.width + value.translation.width,
                                    height: offset.height + value.translation.height), scale: scale, size: geometry.size
                            )
                        }
                    }, including: scale > 1 ? .all : .subviews
                )
                .simultaneousGesture(
                    DragGesture(minimumDistance: 10)
                        .onChanged { value in
                            // A second finger turns the drag into a pinch; don't move the page then.
                            guard magnification == 1 else { return }
                            if !dismissing {
                                guard value.translation.height > abs(value.translation.width) else { return }
                                dismissing = true
                            }
                            transition.dismissDrag(value)
                        }
                        .onEnded { value in
                            guard dismissing else { return }
                            dismissing = false
                            transition.dismissDragEnded(value)
                        }, including: scale > 1 ? .subviews : .all
                )
                .onTapGesture(count: 2) {
                    withAnimation(reduceMotion ? nil : .snappy) {
                        scale = scale > 1 ? 1 : 2
                        offset = .zero
                    }
                }
                .accessibilityAction(named: Text("Reset Zoom")) {
                    scale = 1
                    offset = .zero
                }
                .accessibilityLabel(model.detail?.sticker.title ?? String(localized: "Sticker Preview"))
            }

            if let error = error {
                VStack(spacing: 8) {
                    Text(error).font(.subheadline).foregroundStyle(.secondary)
                    if model.placeholder != nil {
                        Text("Showing a thumbnail only.").font(.caption).foregroundStyle(.secondary)
                    }
                    Button("Retry Preview", action: retry)
                }
            }
        }
    }

    /// The decoded frame, else the core thumbnail, else the grid image the transition started from.
    private var shown: CGImage? { model.playback.frame ?? model.placeholder ?? transition.fallback }

    private func clamped(_ value: CGSize, scale: CGFloat, size: CGSize) -> CGSize {
        guard let image = shown else { return .zero }
        let fit = min(size.width / CGFloat(image.width), size.height / CGFloat(image.height))
        let x = max(0, (CGFloat(image.width) * fit * scale - size.width) / 2)
        let y = max(0, (CGFloat(image.height) * fit * scale - size.height) / 2)
        return CGSize(width: min(x, max(-x, value.width)), height: min(y, max(-y, value.height)))
    }

    private var error: String? {
        if let error = model.originalError { return error }
        if case .failed(let message) = model.playback.state { return message }
        return nil
    }
}
