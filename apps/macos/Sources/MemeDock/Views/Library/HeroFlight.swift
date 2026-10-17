import AppKit
import SwiftUI

/// A frame kept where a view can write it on every layout pass without
/// re-rendering anything: only a flight about to start reads it.
final class FrameBox {
    var rect: CGRect = .zero
}

/// The thumbnail that travels between a grid tile and the detail preview.
///
/// Both ends stay on screen on the desktop and the lazy grid recycles tiles,
/// so the move is drawn by a stand-in above them instead of by matching the
/// two views: the stand-in starts over one end, glides to the other, and is
/// removed once it lands, uncovering the real view underneath.
@MainActor
final class HeroFlight: ObservableObject {
    enum Direction {
        case opening
        case closing
    }

    struct Flight {
        let id: String
        let image: NSImage
        let direction: Direction
        let from: CGRect
        var to: CGRect?
        var landed = false
        /// Tells two flights for the same sticker apart.
        let serial: Int
    }

    /// The coordinate space both ends report in and the stand-in is drawn in.
    nonisolated static let space = "library.hero"

    @Published private(set) var flight: Flight?

    /// The part of the grid that is actually on screen.
    var viewport: CGRect = .zero

    private var tiles: [String: FrameBox] = [:]
    private var preview: CGRect = .zero
    private var serial = 0
    private var pending: Task<Void, Never>?

    /// Whether the detail preview should stay hidden under an arriving stand-in.
    var isOpening: Bool {
        flight?.direction == .opening
    }

    func register(_ id: String, _ box: FrameBox) {
        tiles[id] = box
    }

    func unregister(_ id: String, _ box: FrameBox) {
        if tiles[id] === box { tiles[id] = nil }
    }

    /// Takes off from the tile. The preview has no place yet: the column only
    /// appears once the detail starts loading, and its slot reports in then.
    func open(_ id: String, image: NSImage) {
        guard let from = visibleTile(id) else { return }
        serial += 1
        flight = Flight(id: id, image: image, direction: .opening, from: from, serial: serial)
        let current = serial
        pending?.cancel()
        // A detail that fails to load never shows a preview to land in.
        pending = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 600_000_000)
            guard !Task.isCancelled, let self, self.flight?.serial == current, self.flight?.to == nil else { return }
            self.flight = nil
        }
    }

    /// The detail preview's slot, reported whenever its frame changes. The
    /// first report after a take-off is what the stand-in has been waiting for.
    func reportPreview(_ rect: CGRect) {
        preview = rect
        guard var flight, flight.direction == .opening, flight.to == nil, !rect.isEmpty else { return }
        pending?.cancel()
        flight.to = rect
        self.flight = flight
        land(flight.serial)
    }

    /// Flies back from the preview to the tile, if the tile is on screen. The
    /// tile's place is only known after the grid has re-flowed for the closed
    /// column, so the stand-in waits for that layout before it moves.
    func close(_ id: String, image: NSImage) {
        guard !preview.isEmpty else { return }
        serial += 1
        let current = serial
        flight = Flight(id: id, image: image, direction: .closing, from: preview, serial: current)
        preview = .zero
        pending?.cancel()
        pending = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 40_000_000)
            guard !Task.isCancelled, let self, var flight = self.flight, flight.serial == current else { return }
            guard let to = self.visibleTile(id) else {
                self.flight = nil
                return
            }
            flight.to = to
            self.flight = flight
            self.land(current)
        }
    }

    func cancel() {
        pending?.cancel()
        flight = nil
    }

    private func land(_ current: Int) {
        withAnimation(MemeDockMotion.panel) {
            flight?.landed = true
        } completion: { [weak self] in
            guard let self, self.flight?.serial == current else { return }
            self.flight = nil
        }
    }

    /// A tile counts only while most of it is on screen: the lazy grid keeps a
    /// few rows beyond the edges, and flying into one of those would end
    /// behind the scroll view's clip.
    private func visibleTile(_ id: String) -> CGRect? {
        guard let rect = tiles[id]?.rect, !rect.isEmpty else { return nil }
        let shown = viewport.intersection(rect)
        guard !shown.isNull, shown.height >= rect.height / 2 else { return nil }
        return rect
    }
}

/// Draws the travelling thumbnail. It never takes clicks, so the views under
/// it keep working while it moves.
struct HeroOverlay: View {
    @ObservedObject var hero: HeroFlight

    var body: some View {
        if let flight = hero.flight {
            let rect = flight.landed ? (flight.to ?? flight.from) : flight.from
            Image(nsImage: flight.image)
                .resizable()
                .interpolation(.high)
                .scaledToFit()
                .padding(4)
                .frame(width: rect.width, height: rect.height)
                .position(x: rect.midX, y: rect.midY)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
        }
    }
}

extension View {
    /// Hands this view's frame in the hero space to `action` after every
    /// layout that moves it.
    func onHeroFrame(_ action: @escaping (CGRect) -> Void) -> some View {
        onGeometryChange(for: CGRect.self) { proxy in
            proxy.frame(in: .named(HeroFlight.space))
        } action: { rect in
            action(rect)
        }
    }

    /// Writes this view's frame into `box`, which re-renders nothing.
    func heroFrame(_ box: FrameBox) -> some View {
        onHeroFrame { box.rect = $0 }
    }

    /// Reports the visible part of the grid, which decides whether a tile is
    /// on screen enough to fly to.
    func heroViewport(_ hero: HeroFlight) -> some View {
        onHeroFrame { hero.viewport = $0 }
    }
}
