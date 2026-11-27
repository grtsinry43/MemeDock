import AVKit
import Combine
import Observation
import SwiftUI

/// Owns playback observation, smooth 30 Hz playhead tracking, seamless looping
/// within `[lower, upper]`, and serialized seeks while a handle or timeline is dragged.
/// Automatically pauses when the app leaves the foreground or the window is hidden.
@MainActor @Observable
final class CreatorPlayer {
    let player = AVPlayer()
    private(set) var position = 0.0
    private(set) var playing = false
    private(set) var available = true
    @ObservationIgnored private var observer: Any?
    @ObservationIgnored private var endObserver: NSObjectProtocol?
    @ObservationIgnored private var lifecycleObservers: Set<AnyCancellable> = []
    @ObservationIgnored private var desiredTime: Double?
    @ObservationIgnored private var seeking = false
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var lower = 0.0
    @ObservationIgnored private var upper = 0.0
    @ObservationIgnored private var totalDuration = 0.0

    init() {
        let center = NotificationCenter.default
        center.publisher(for: NSApplication.didResignActiveNotification)
            .sink { [weak self] _ in self?.pause() }
            .store(in: &lifecycleObservers)
        center.publisher(for: NSApplication.didHideNotification)
            .sink { [weak self] _ in self?.pause() }
            .store(in: &lifecycleObservers)
        NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.willSleepNotification)
            .sink { [weak self] _ in self?.pause() }
            .store(in: &lifecycleObservers)
    }

    func setWindowVisible(_ visible: Bool) {
        if !visible { pause() }
    }

    func load(path: String, mime: String, duration: Double, end: Double) {
        clear()
        totalDuration = max(0, duration)
        let asset = AVURLAsset(url: URL(fileURLWithPath: path), options: [AVURLAssetOverrideMIMETypeKey: mime])
        let item = AVPlayerItem(asset: asset)
        player.replaceCurrentItem(with: item)
        let token = generation
        Task { [weak self] in
            let playable = (try? await asset.load(.isPlayable)) ?? false
            guard let self, token == self.generation else { return }
            self.available = playable
        }
        setRange(start: 0, end: end)
        endObserver = NotificationCenter.default.addObserver(
            forName: .AVPlayerItemDidPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self, self.playing else { return }
                self.loopToStart()
            }
        }
        observer = player.addPeriodicTimeObserver(
            forInterval: CMTime(value: 1, timescale: 30),
            queue: .main
        ) { [weak self] time in
            Task { @MainActor in
                guard let self else { return }
                let seconds = time.seconds
                if seconds.isFinite && !self.seeking {
                    self.position = seconds
                }
                if self.playing && !self.seeking && seconds >= self.upper - 0.025 {
                    self.loopToStart()
                }
            }
        }
    }

    func setRange(start: Double, end: Double) {
        lower = start
        upper = end
        if position < start || position > end {
            seek(start)
        }
    }

    func seek(_ seconds: Double) {
        let clamped = max(0, totalDuration > 0 ? min(totalDuration, seconds) : seconds)
        guard available else {
            position = clamped
            return
        }
        desiredTime = clamped
        position = clamped
        chaseSeek()
    }

    func step(by delta: Double) {
        pause()
        seek(position + delta)
    }

    private func loopToStart() {
        guard available, playing else { return }
        desiredTime = lower
        position = lower
        chaseSeek(resumeAfter: true)
    }

    private func chaseSeek(resumeAfter: Bool = false) {
        guard !seeking, let seconds = desiredTime, player.currentItem != nil else { return }
        desiredTime = nil
        seeking = true
        let token = generation
        player.seek(
            to: CMTime(seconds: seconds, preferredTimescale: 1000),
            toleranceBefore: .zero,
            toleranceAfter: .zero
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self, token == self.generation else { return }
                self.seeking = false
                if resumeAfter && self.playing && self.player.rate == 0 {
                    self.player.play()
                }
                self.chaseSeek(resumeAfter: resumeAfter)
            }
        }
    }

    func toggle() {
        guard available else { return }
        if playing {
            pause()
        } else {
            if position >= upper - 0.025 || position < lower {
                seek(lower)
            }
            playing = true
            player.play()
        }
    }

    func pause() {
        player.pause()
        playing = false
    }

    func clear() {
        generation += 1
        pause()
        if let observer { player.removeTimeObserver(observer) }
        observer = nil
        if let endObserver { NotificationCenter.default.removeObserver(endObserver) }
        endObserver = nil
        player.replaceCurrentItem(with: nil)
        desiredTime = nil
        seeking = false
        position = 0
        totalDuration = 0
        available = true
    }
}

struct CreatorVideoPlayer: NSViewRepresentable {
    let player: AVPlayer

    func makeNSView(context: Context) -> AVPlayerView {
        let view = AVPlayerView()
        view.controlsStyle = .none
        view.videoGravity = .resizeAspect
        view.player = player
        return view
    }

    func updateNSView(_ view: AVPlayerView, context: Context) {
        view.player = player
    }

    static func dismantleNSView(_ view: AVPlayerView, coordinator: ()) {
        view.player = nil
    }
}
