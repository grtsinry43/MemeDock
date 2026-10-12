import AppKit
import Combine
import Foundation
import ImageIO

/// Plays an animated GIF or WebP for the detail pane, matching what the Linux
/// client does: playback starts as soon as the sticker is shown, frames are
/// decoded one at a time so a long animation never needs its whole sequence in
/// memory, the sequence loops, and pausing drops the decoder so resuming starts
/// from the first frame again.
///
/// PNG animations are excluded on purpose: the core reports them as animated,
/// but the Linux client only ever shows their first frame, so the pane says so
/// instead of appearing to play them.
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

    /// Formats whose animation this pane plays.
    static func plays(animated: Bool, mime: String) -> Bool {
        animated && (mime == "image/gif" || mime == "image/webp")
    }

    /// An animation the pane reports but does not play, so the pane can explain
    /// that the shown frame is the only one.
    static func isStillAnimation(animated: Bool, mime: String) -> Bool {
        animated && mime == "image/png"
    }

    func load(path: String?, animated: Bool, mime: String) {
        stop()
        self.path = nil
        frame = nil
        state = .idle
        guard let path, Self.plays(animated: animated, mime: mime) else { return }
        self.path = path
        start(path: path)
    }

    func stop() {
        work?.cancel()
        work = nil
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
            start(path: path)
        case .idle, .broken, .limit:
            break
        }
    }

    private func start(path: String) {
        work?.cancel()
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
