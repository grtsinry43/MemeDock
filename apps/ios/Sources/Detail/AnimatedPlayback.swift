import CoreGraphics
import Foundation
import MemeDockBridge
import Observation

@MainActor
@Observable
final class AnimatedPlayback {
    enum State: Equatable {
        case idle, loading, still, playing, paused, finished
        case failed(String)
    }

    private struct Configuration {
        let path: String
        let asset: AssetMetadata
        let limits: ResourceConfiguration
        let pixels: Int
    }

    private(set) var frame: CGImage?
    private(set) var state: State = .idle
    private(set) var frameIndex = 0
    private(set) var description: PreviewDescription?
    @ObservationIgnored private let decoder = PreviewDecoder()
    @ObservationIgnored private var configuration: Configuration?
    @ObservationIgnored private var prepareWork: Task<Void, Never>?
    @ObservationIgnored private var presentationWork: Task<Void, Never>?
    @ObservationIgnored private var loop: Task<Void, Never>?
    @ObservationIgnored private var cleanup: Task<Void, Never>?
    @ObservationIgnored private var token = UUID()
    @ObservationIgnored private var active = true
    @ObservationIgnored private var reduceMotion = false
    @ObservationIgnored private var wantsPlayback = true
    @ObservationIgnored private var explicitlyStarted = false
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var completedPlays = 0

    var hasConfiguration: Bool { configuration != nil }
    var isAnimated: Bool { (description?.frameCount ?? 0) > 1 }

    func load(path: String, asset: AssetMetadata, limits: ResourceConfiguration, pixels: Int) async {
        guard !closed, !Task.isCancelled else { return }
        presentationWork?.cancel()
        cancelWork(clearFrame: true)
        configuration = Configuration(path: path, asset: asset, limits: limits, pixels: pixels)
        frameIndex = 0
        completedPlays = 0
        description = nil
        guard active else { state = .paused; return }
        await prepare()
    }

    func setPresentation(active: Bool, reduceMotion: Bool) {
        let changed = self.active != active || self.reduceMotion != reduceMotion
        self.active = active
        self.reduceMotion = reduceMotion
        guard changed, !closed else { return }
        if !active {
            presentationWork?.cancel()
            cancelWork(clearFrame: true)
            if configuration != nil { state = .paused }
        } else if reduceMotion, !explicitlyStarted, state == .playing {
            cancelWork(clearFrame: false)
            state = .paused
        } else if configuration != nil {
            prepareWork?.cancel()
            presentationWork?.cancel()
            presentationWork = Task { [weak self] in await self?.prepare() }
        }
    }

    func toggle() async {
        guard !closed, active, isAnimated, state != .loading else { return }
        if state == .playing {
            wantsPlayback = false
            explicitlyStarted = false
            cancelWork(clearFrame: false)
            state = .paused
        } else {
            wantsPlayback = true
            explicitlyStarted = true
            if state == .finished { frameIndex = 0; completedPlays = 0 }
            await prepare()
        }
    }

    func reset() {
        cancelWork(clearFrame: true)
        configuration = nil
        description = nil
        state = .idle
    }

    func shutdown() async {
        closed = true
        active = false
        let preparing = prepareWork
        let presenting = presentationWork
        presenting?.cancel()
        let playing = loop
        reset()
        await preparing?.value
        await presenting?.value
        await playing?.value
        await cleanup?.value
    }

    func waitUntilPrepared() async {
        await presentationWork?.value
        await prepareWork?.value
    }

    private func cancelWork(clearFrame: Bool) {
        prepareWork?.cancel()
        loop?.cancel()
        let previous = token
        token = UUID()
        cleanup = Task { await decoder.close(token: previous) }
        if clearFrame { frame = nil }
    }

    private func prepare() async {
        guard !closed, active, !Task.isCancelled, let configuration else { return }
        cancelWork(clearFrame: false)
        let current = token
        state = .loading
        prepareWork = Task { [weak self] in
            guard let self else { return }
            do {
                let description = try await decoder.open(
                    path: configuration.path, asset: configuration.asset, limits: configuration.limits,
                    pixels: configuration.pixels, token: current
                )
                let index = min(frameIndex, description.frameCount - 1)
                let image = try await decoder.frame(at: index, token: current)
                guard token == current, !closed, active, !Task.isCancelled else { return }
                self.description = description
                frameIndex = index
                frame = image
                if description.frameCount == 1 {
                    state = .still
                    await decoder.close(token: current)
                } else if let plays = description.totalPlays, completedPlays >= plays {
                    state = .finished
                    await decoder.close(token: current)
                } else if wantsPlayback && (explicitlyStarted || !reduceMotion) {
                    state = .playing
                    startLoop(description, token: current)
                } else {
                    state = .paused
                    await decoder.close(token: current)
                }
            } catch {
                guard token == current, !closed, !Task.isCancelled else { return }
                frame = nil
                state = .failed(BridgeErrors.previewDescription(error))
                await decoder.close(token: current)
            }
        }
        let operation = prepareWork
        await withTaskCancellationHandler {
            await operation?.value
        } onCancel: {
            Task { @MainActor [weak self] in
                guard let self, self.token == current else { return }
                self.cancelWork(clearFrame: false)
            }
        }
    }

    private func startLoop(_ description: PreviewDescription, token current: UUID) {
        loop = Task { [weak self] in
            guard let self else { return }
            let clock = ContinuousClock()
            var index = frameIndex
            var deadline = clock.now + .seconds(description.delays[index])
            do {
                while true {
                    try await clock.sleep(until: deadline)
                    try Task.checkCancellation()
                    guard token == current, active, !closed else { return }
                    index += 1
                    if index == description.frameCount {
                        completedPlays += 1
                        if let plays = description.totalPlays, completedPlays >= plays {
                            if frameIndex != description.frameCount - 1 {
                                let last = try await decoder.frame(at: description.frameCount - 1, token: current)
                                guard token == current, active, !closed, !Task.isCancelled else { return }
                                frame = last
                                frameIndex = description.frameCount - 1
                            }
                            state = .finished
                            await decoder.close(token: current)
                            return
                        }
                        index = 0
                    }
                    deadline += .seconds(description.delays[index])
                    // Skip overdue presentation slots instead of building a frame backlog.
                    if clock.now >= deadline { continue }
                    let image = try await decoder.frame(at: index, token: current)
                    guard token == current, active, !closed, !Task.isCancelled else { return }
                    frame = image
                    frameIndex = index
                }
            } catch {
                guard token == current, !closed, !Task.isCancelled else { return }
                frame = nil
                state = .failed(BridgeErrors.previewDescription(error))
                await decoder.close(token: current)
            }
        }
    }
}
