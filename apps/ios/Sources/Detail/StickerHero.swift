import Accessibility
import CoreGraphics
import Observation
import SwiftUI

/// The sticker detail layer above the tabs. Only the image travels between a grid cell and the
/// detail preview; the page fades and its bars slide in separately.
@MainActor @Observable
final class StickerHero {
    struct Presentation {
        let route: StickerDetailRoute
        let image: CGImage?
        let source: CGRect?
    }
    private(set) var presentation: Presentation?
    /// Grid cells currently on screen, keyed by sticker ID. Updated on every scroll frame, so not observed.
    @ObservationIgnored private var frames: [String: CGRect] = [:]

    func present(_ route: StickerDetailRoute, image: CGImage?) {
        guard presentation == nil else { return }
        presentation = Presentation(route: route, image: image, source: frames[route.id])
    }

    /// Removes the layer without motion, for programmatic navigation and library resets.
    func reset() { presentation = nil }

    func register(_ id: String, frame: CGRect) { frames[id] = frame }
    func unregister(_ id: String) { frames[id] = nil }
    func frame(for id: String) -> CGRect? { frames[id] }
}

/// Lets a grid cell act as the origin and return target of the detail image.
struct StickerHeroSource: ViewModifier {
    let id: String
    let hero: StickerHero?

    func body(content: Content) -> some View {
        if let hero {
            content
                .opacity(hero.presentation?.route.id == id ? 0 : 1)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { hero.register(id, frame: $0) }
                .onDisappear { hero.unregister(id) }
        } else {
            content
        }
    }
}

/// Transition state the detail page reads to place its bars and preview.
struct DetailTransition {
    var chromeVisible = true
    var previewHidden = false
    /// Open and at rest: animation playback may run.
    var settled = true
    var dragging = false
    var fallback: CGImage?
    var previewFrame: (CGRect) -> Void = { _ in }
    var dismissDrag: (DragGesture.Value) -> Void = { _ in }
    var dismissDragEnded: (DragGesture.Value) -> Void = { _ in }
    var dismiss: () -> Void = {}
}

struct StickerHeroOverlay: View {
    let hero: StickerHero
    let presentation: StickerHero.Presentation
    let session: LibrarySession
    private enum Phase { case opening, open, closing }
    private enum DragAxis { case vertical, edge }
    @State private var model: StickerDetailModel?
    @State private var phase = Phase.opening
    /// False when the page only fades: Reduce Motion, or no visible cell to fly from or back to.
    @State private var flies = true
    @State private var expanded = false
    @State private var expanding = false
    @State private var chrome = false
    @State private var target: CGRect?
    @State private var closingTarget: CGRect?
    @State private var drag: CGSize = .zero
    @State private var axis: DragAxis?
    @State private var bounds: CGRect = .zero
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let open = Animation.spring(duration: 0.45, bounce: 0.2)
    private static let close = Animation.spring(duration: 0.4, bounce: 0.12)
    private static let settle = Animation.spring(duration: 0.35, bounce: 0.25)
    private static let fade = Animation.easeOut(duration: 0.2)

    var body: some View {
        GeometryReader { proxy in
            let origin = proxy.frame(in: .global).origin
            ZStack {
                Color(uiColor: .systemBackground).opacity(backgroundOpacity).ignoresSafeArea()
                StickerDetailView(session: session, model: model, transition: transition)
                    .opacity(contentOpacity)
                    .allowsHitTesting(phase == .open && axis == nil)
                // Stays mounted while open so a dismissal starts moving from the preview's frame.
                if let image = flyingImage, let rect = flyingRect {
                    Image(decorative: image, scale: 1).resizable().scaledToFit()
                        .frame(width: rect.width, height: rect.height)
                        .position(x: rect.midX - origin.x, y: rect.midY - origin.y)
                        .animation(nil) { $0.opacity(flying ? 1 : 0) }
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
                edgeSwipe
            }
        }
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { bounds = $0 }
        .accessibilityAddTraits(.isModal)
        .accessibilityAction(.escape) { dismiss() }
        .onAppear {
            if model == nil { model = session.makeDetail(for: presentation.route) }
            if reduceMotion || presentation.source == nil || presentation.image == nil || model == nil {
                reveal()
            }
        }
        .task {
            // A page without a preview (missing or failed sticker) never reports a target.
            try? await Task.sleep(for: .milliseconds(300))
            if phase == .opening, !expanding, !expanded { reveal() }
        }
    }

    private var transition: DetailTransition {
        DetailTransition(
            chromeVisible: chrome,
            previewHidden: flying,
            settled: phase == .open && axis == nil,
            dragging: axis != nil,
            fallback: presentation.image,
            previewFrame: { frame in
                target = frame
                if phase == .opening, flies, !expanding { expand() }
            },
            dismissDrag: { value in
                guard phase == .open, axis != .edge else { return }
                if axis == nil { flies = !reduceMotion }
                axis = .vertical
                drag = value.translation
            },
            dismissDragEnded: { value in
                guard axis == .vertical else { return }
                finishDrag(commit: value.translation.height > 100 || value.predictedEndTranslation.height > 260)
            },
            dismiss: dismiss)
    }

    /// The traveling copy stands in for the preview while it moves.
    private var flying: Bool { flies && (phase != .open || axis != nil) }

    private var flyingImage: CGImage? {
        guard flies else { return nil }
        if phase == .opening { return presentation.image }
        return model?.playback.frame ?? model?.placeholder ?? presentation.image
    }

    private var flyingRect: CGRect? {
        switch phase {
        case .opening:
            return expanded ? target : presentation.source
        case .closing:
            return closingTarget
        case .open:
            guard let target else { return nil }
            let scale = 1 - 0.45 * progress
            let size = CGSize(width: target.width * scale, height: target.height * scale)
            return CGRect(
                x: target.midX + drag.width - size.width / 2, y: target.midY + drag.height - size.height / 2,
                width: size.width, height: size.height)
        }
    }

    private var progress: CGFloat {
        switch axis {
        case .vertical: min(1, max(0, drag.height) / max(1, bounds.height * 0.45))
        case .edge: min(1, max(0, drag.width) / max(1, bounds.width * 0.6))
        case nil: 0
        }
    }

    private var backgroundOpacity: Double {
        switch phase {
        case .opening: expanded ? 1 : 0
        case .closing: 0
        case .open: 1 - 0.9 * progress
        }
    }

    private var contentOpacity: Double {
        switch phase {
        case .opening: expanded ? 1 : 0
        case .closing: 0
        case .open: 1 - min(1, 2 * progress)
        }
    }

    private var edgeSwipe: some View {
        Color.clear
            .frame(width: 20)
            .contentShape(.rect)
            .padding(.top, 120)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .gesture(
                DragGesture(minimumDistance: 8, coordinateSpace: .global)
                    .onChanged { value in
                        guard phase == .open, axis != .vertical else { return }
                        if axis == nil { flies = !reduceMotion }
                        axis = .edge
                        drag = value.translation
                    }
                    .onEnded { value in
                        guard axis == .edge else { return }
                        finishDrag(
                            commit: value.translation.width > bounds.width * 0.3
                                || value.predictedEndTranslation.width > bounds.width * 0.6)
                    }
            )
            .allowsHitTesting(phase == .open)
            .accessibilityHidden(true)
    }

    private func expand() {
        expanding = true
        // The page's first layout is expensive; starting the spring in that same frame skips its beginning.
        Task {
            try? await Task.sleep(for: .milliseconds(16))
            guard phase == .opening, !expanded else { return }
            withAnimation(Self.open) { expanded = true } completion: { settle() }
            withAnimation(Self.open.delay(0.05)) { chrome = true }
        }
    }

    private func reveal() {
        guard phase == .opening, !expanded else { return }
        flies = false
        withAnimation(Self.fade) {
            expanded = true
            chrome = true
        } completion: { settle() }
    }

    private func settle() {
        guard phase == .opening else { return }
        phase = .open
        AccessibilityNotification.ScreenChanged().post()
    }

    private func finishDrag(commit: Bool) {
        if commit {
            dismiss()
        } else {
            withAnimation(Self.settle) { drag = .zero } completion: { if phase == .open { axis = nil } }
        }
    }

    private func dismiss() {
        guard phase == .open else { return }
        let destination = hero.frame(for: presentation.route.id).flatMap { bounds.intersects($0) ? $0 : nil }
        guard flies, let destination, target != nil else {
            flies = false
            withAnimation(Self.fade) {
                phase = .closing
                chrome = false
            } completion: { hero.reset() }
            return
        }
        withAnimation(Self.close) {
            phase = .closing
            closingTarget = destination
            chrome = false
            drag = .zero
        } completion: { hero.reset() }
    }
}
