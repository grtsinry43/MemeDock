import AppKit
import Combine
import Foundation
import ImageIO
import SwiftUI

/// Plays an animated GIF or WebP for the detail pane and Creator result view:
/// frames are decoded one at a time off the main actor, and playback suspends
/// automatically whenever the app is inactive, hidden, asleep, or its hosting
/// window is occluded or minimized.
@MainActor
final class AnimatedPlayback: ObservableObject {
    enum State: Equatable {
        /// Nothing to play: a still image, or a file the core calls still.
        case idle
        case playing
        case paused
        /// The file could not be decoded as an animation.
        case broken
        /// The image is past the frame limits the other clients enforce.
        case limit
    }

    /// The longest edge of a decoded frame; the Linux client uses the same bound.
    private nonisolated static let edge = 1280
    /// Frame limits taken from the core defaults and applied the way the Linux
    /// client applies them: either too many pixels per frame, or a frame that
    /// would need more memory than a single decode may reserve.
    private nonisolated static let maxFramePixels: UInt64 = 16_000_000
    private nonisolated static let maxDecodeBytes: UInt64 = 128 * 1024 * 1024
    /// A frame without a usable delay waits this long, as the Linux client does.
    private nonisolated static let fallbackDelay: TimeInterval = 0.1

    @Published private(set) var frame: NSImage?
    @Published private(set) var state: State = .idle

    private var path: String?
    private var work: Task<Void, Never>?
    private var suspended = false
    private var windowVisible = true
    private var asleep = false
    private var observers: Set<AnyCancellable> = []

    init() {
        let center = NotificationCenter.default
        center.publisher(for: NSApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in self?.reconcileActivity() }
            .store(in: &observers)
        center.publisher(for: NSApplication.didResignActiveNotification)
            .sink { [weak self] _ in self?.reconcileActivity() }
            .store(in: &observers)
        center.publisher(for: NSApplication.didHideNotification)
            .sink { [weak self] _ in self?.reconcileActivity() }
            .store(in: &observers)
        center.publisher(for: NSApplication.didUnhideNotification)
            .sink { [weak self] _ in self?.reconcileActivity() }
            .store(in: &observers)
        let workspace = NSWorkspace.shared.notificationCenter
        workspace.publisher(for: NSWorkspace.willSleepNotification)
            .sink { [weak self] _ in self?.asleep = true; self?.reconcileActivity() }
            .store(in: &observers)
        workspace.publisher(for: NSWorkspace.didWakeNotification)
            .sink { [weak self] _ in self?.asleep = false; self?.reconcileActivity() }
            .store(in: &observers)
    }

    private var canAnimate: Bool {
        NSApp.isActive && !NSApp.isHidden && !asleep && windowVisible
    }

    /// Formats whose animation this pane plays.
    static func plays(animated: Bool, mime: String) -> Bool {
        animated && (mime == "image/gif" || mime == "image/webp")
    }

    /// An animation the pane reports but does not play, so the pane can explain
    /// that the shown frame is the only one.
    static func isStillAnimation(animated: Bool, mime: String) -> Bool {
        animated && mime == "image/png"
    }

    func setWindowVisible(_ visible: Bool) {
        guard windowVisible != visible else { return }
        windowVisible = visible
        reconcileActivity()
    }

    func load(path: String?, animated: Bool, mime: String) {
        stop()
        self.path = nil
        frame = nil
        state = .idle
        suspended = false
        guard let path, Self.plays(animated: animated, mime: mime) else { return }
        self.path = path
        if canAnimate {
            start(path: path)
        } else {
            state = .playing
            suspended = true
            decodeFirstFrame(path: path)
        }
    }

    func stop() {
        work?.cancel()
        work = nil
        suspended = false
    }

    /// Pausing drops the decoder and resuming replays from the first frame, which
    /// is what the Linux client's play button does when it restarts playback.
    func toggle() {
        switch state {
        case .playing:
            stop()
            state = .paused
        case .paused:
            guard let path else { return }
            if canAnimate {
                start(path: path)
            } else {
                state = .playing
                suspended = true
            }
        case .idle, .broken, .limit:
            break
        }
    }

    private func reconcileActivity() {
        guard let path else { return }
        if canAnimate {
            if state == .playing, suspended {
                suspended = false
                start(path: path)
            }
        } else if state == .playing, !suspended {
            work?.cancel()
            work = nil
            suspended = true
        }
    }

    private func decodeFirstFrame(path: String) {
        work?.cancel()
        work = Task { [weak self] in
            let outcome = await Task.detached(priority: .utility) { () -> Result<CGImage, PlaybackFault> in
                let url = URL(fileURLWithPath: path) as CFURL
                guard let source = CGImageSourceCreateWithURL(url, nil) else { return .failure(.broken) }
                guard CGImageSourceGetCount(source) > 1 else { return .failure(.broken) }
                guard let size = Self.frameSize(source, index: 0) else { return .failure(.broken) }
                guard !Self.exceeds(size) else { return .failure(.limit) }
                guard let image = Self.decode(source, index: 0) else { return .failure(.broken) }
                return .success(image)
            }.value
            guard let self, !Task.isCancelled, self.path == path else { return }
            switch outcome {
            case let .success(decoded):
                self.frame = NSImage(cgImage: decoded, size: NSSize(width: decoded.width, height: decoded.height))
            case let .failure(fault):
                self.frame = nil
                self.suspended = false
                self.state = fault == .limit ? .limit : .broken
            }
        }
    }

    private func start(path: String) {
        work?.cancel()
        suspended = false
        state = .playing
        work = Task { [weak self] in
            while !Task.isCancelled {
                do {
                    for try await decoded in Self.frames(path: path) {
                        guard let self, !Task.isCancelled else { return }
                        self.frame = NSImage(
                            cgImage: decoded,
                            size: NSSize(width: decoded.width, height: decoded.height)
                        )
                    }
                } catch let fault as PlaybackFault {
                    guard let self, !Task.isCancelled else { return }
                    self.frame = nil
                    self.state = fault == .limit ? .limit : .broken
                    return
                } catch {
                    // Cancellation and stream teardown both land here; the task
                    // that owns the state is already gone.
                    return
                }
            }
        }
    }

    /// Streams one cycle of the animation. Frames are decoded off the main actor
    /// and a frame is only produced once the previous frame's delay has elapsed,
    /// so a slow source cannot queue work up behind itself.
    private nonisolated static func frames(path: String) -> AsyncThrowingStream<CGImage, Error> {
        AsyncThrowingStream { continuation in
            let worker = Task.detached(priority: .userInitiated) {
                do {
                    try await decodeCycle(path: path, into: continuation)
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in worker.cancel() }
        }
    }

    private nonisolated static func decodeCycle(
        path: String,
        into continuation: AsyncThrowingStream<CGImage, Error>.Continuation
    ) async throws {
        let url = URL(fileURLWithPath: path) as CFURL
        guard let source = CGImageSourceCreateWithURL(url, nil) else { throw PlaybackFault.broken }
        let count = CGImageSourceGetCount(source)
        // One frame is a still image; the core reports those as not animated.
        guard count > 1 else { throw PlaybackFault.broken }
        guard let size = frameSize(source, index: 0) else { throw PlaybackFault.broken }
        guard !exceeds(size) else { throw PlaybackFault.limit }
        for index in 0..<count {
            try Task.checkCancellation()
            guard let image = decode(source, index: index) else { throw PlaybackFault.broken }
            let wait = delay(source, index: index)
            continuation.yield(image)
            try await Task.sleep(nanoseconds: UInt64(wait * 1_000_000_000))
        }
    }

    /// Decoding through the thumbnail path keeps every frame inside the edge
    /// bound, which is the same guarantee the Linux client gets when it fits a
    /// frame before handing it to the compositor.
    private nonisolated static func decode(_ source: CGImageSource, index: Int) -> CGImage? {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceThumbnailMaxPixelSize: edge,
            kCGImageSourceShouldCacheImmediately: true,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, index, options as CFDictionary)
    }

    private nonisolated static func frameSize(
        _ source: CGImageSource,
        index: Int
    ) -> (width: UInt64, height: UInt64)? {
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any],
              let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.uint64Value,
              let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.uint64Value,
              width > 0,
              height > 0
        else { return nil }
        return (width, height)
    }

    /// The unclamped delay is the stored one; the clamped value is the fallback
    /// for formats that only report that.
    private nonisolated static func delay(_ source: CGImageSource, index: Int) -> TimeInterval {
        let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any]
        let gif = properties?[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        let webp = properties?[kCGImagePropertyWebPDictionary] as? [CFString: Any]
        let seconds = number(gif?[kCGImagePropertyGIFUnclampedDelayTime])
            ?? number(webp?[kCGImagePropertyWebPUnclampedDelayTime])
            ?? number(gif?[kCGImagePropertyGIFDelayTime])
            ?? number(webp?[kCGImagePropertyWebPDelayTime])
        guard let seconds, seconds > 0 else { return fallbackDelay }
        return seconds
    }

    private nonisolated static func number(_ value: Any?) -> TimeInterval? {
        (value as? NSNumber)?.doubleValue
    }

    private nonisolated static func exceeds(_ size: (width: UInt64, height: UInt64)) -> Bool {
        let pixels = size.width.multipliedReportingOverflow(by: size.height)
        let value = pixels.partialValue
        let bytes = value.multipliedReportingOverflow(by: 16)
        return pixels.overflow
            || value == 0
            || value > maxFramePixels
            || bytes.overflow
            || bytes.partialValue > maxDecodeBytes
    }
}

/// Decoder outcomes the pane turns into a message.
private enum PlaybackFault: Error {
    case broken
    case limit
}

extension View {
    /// Reports whether the hosting `NSWindow` is currently un-minimized and
    /// un-occluded according to the WindowServer.
    func onWindowVisibilityChange(_ perform: @escaping (Bool) -> Void) -> some View {
        background(WindowVisibilityBridge(onChange: perform))
    }
}

private struct WindowVisibilityBridge: NSViewRepresentable {
    let onChange: (Bool) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onChange: onChange) }

    func makeNSView(context: Context) -> ObserverView {
        let view = ObserverView()
        view.onWindowChange = { [weak coordinator = context.coordinator] window in
            coordinator?.attach(to: window)
        }
        return view
    }

    func updateNSView(_ view: ObserverView, context: Context) {
        context.coordinator.onChange = onChange
        context.coordinator.attach(to: view.window)
    }

    static func dismantleNSView(_ view: ObserverView, coordinator: Coordinator) {
        coordinator.detach()
    }

    final class ObserverView: NSView {
        var onWindowChange: ((NSWindow?) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            onWindowChange?(window)
        }
    }

    @MainActor
    final class Coordinator {
        var onChange: (Bool) -> Void
        private weak var observedWindow: NSWindow?
        private var tokens: [NSObjectProtocol] = []
        private var lastVisible: Bool?

        init(onChange: @escaping (Bool) -> Void) {
            self.onChange = onChange
        }

        func attach(to window: NSWindow?) {
            guard observedWindow !== window else { return }
            detach()
            observedWindow = window
            guard let window else {
                report(false)
                return
            }
            let center = NotificationCenter.default
            let names: [NSNotification.Name] = [
                NSWindow.didChangeOcclusionStateNotification,
                NSWindow.didMiniaturizeNotification,
                NSWindow.didDeminiaturizeNotification,
            ]
            for name in names {
                let token = center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                    Task { @MainActor in self?.evaluate() }
                }
                tokens.append(token)
            }
            evaluate()
        }

        func detach() {
            let center = NotificationCenter.default
            for token in tokens { center.removeObserver(token) }
            tokens.removeAll()
            observedWindow = nil
        }

        private func evaluate() {
            guard let window = observedWindow else {
                report(false)
                return
            }
            let visible = !window.isMiniaturized && window.occlusionState.contains(.visible)
            report(visible)
        }

        private func report(_ visible: Bool) {
            guard lastVisible != visible else { return }
            lastVisible = visible
            onChange(visible)
        }
    }
}
