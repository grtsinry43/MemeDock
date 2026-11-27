import AppKit
import Combine
import MemeDockBridge
import Observation
import UniformTypeIdentifiers

struct CreatorThumbnail: Identifiable {
    let id: UInt64
    let lease: ArtifactLeaseHandle
    let image: NSImage
}

@MainActor @Observable
final class CreatorModel {
    let playback = CreatorPlayer()
    private(set) var source: CreatorVideoHandle?
    private(set) var info: VideoInfo?
    private(set) var name = ""
    private(set) var thumbnails: [CreatorThumbnail] = []
    private(set) var fallbackImage: NSImage?
    private(set) var result: CreatedAnimation?
    private(set) var busy = false
    private(set) var saving = false
    private(set) var failure: String?
    private(set) var stage = ""
    private(set) var stale = false
    var start = 0.0 { didSet { if start != oldValue { invalidate() } } }
    var end = 1.0 { didSet { if end != oldValue { invalidate() } } }
    var crop = CGRect(x: 0, y: 0, width: 1, height: 1) { didSet { if crop != oldValue { invalidate() } } }
    var aspectLock: Double? = nil
    var edge = 512 { didSet { if edge != oldValue { invalidate() } } }
    var fps = 15 { didSet { if fps != oldValue { invalidate() } } }
    var quality = 80.0 { didSet { if quality != oldValue { invalidate() } } }
    var showingResult = false
    var modeIsCreator = false { didSet { if !modeIsCreator { playback.pause() } } }

    @ObservationIgnored private let session: LibrarySession
    @ObservationIgnored private let actions: StickerActions
    @ObservationIgnored private let status: StatusCenter
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var fallbackWork: Task<Void, Never>?
    @ObservationIgnored private var fallbackTask: CreatorArtifactTask?
    @ObservationIgnored private var fallbackLease: ArtifactLeaseHandle?
    @ObservationIgnored private var progressWork: Task<Void, Never>?
    @ObservationIgnored private var openTask: CreatorVideoTask?
    @ObservationIgnored private var renderTask: CreatorAnimationTask?
    @ObservationIgnored private var previewTask: CreatorArtifactTask?
    @ObservationIgnored private var revision = 0
    @ObservationIgnored private var subscriptions: Set<AnyCancellable> = []

    init(session: LibrarySession, actions: StickerActions, status: StatusCenter) {
        self.session = session; self.actions = actions; self.status = status
        session.$phase.sink { [weak self] phase in
            if phase != .ready { self?.shutdown() }
        }.store(in: &subscriptions)
    }

    var duration: Double { Double(info?.durationMs ?? 0) / 1000 }
    var canGenerate: Bool { source != nil && !busy && end > start && end - start <= 30.0001 }
    var output: ArtifactLeaseHandle? { result?.artifact }
    var hasCurrentResult: Bool { result != nil && !stale }
    var isCropped: Bool {
        abs(crop.minX) > 0.0005 || abs(crop.minY) > 0.0005
            || abs(crop.width - 1) > 0.0005 || abs(crop.height - 1) > 0.0005
    }
    var croppedPixelSize: (width: Int, height: Int)? {
        guard let info else { return nil }
        let w = max(1, Int((Double(info.width) * crop.width).rounded()))
        let h = max(1, Int((Double(info.height) * crop.height).rounded()))
        return (w, h)
    }
    var estimatedOutputSize: (width: Int, height: Int)? {
        guard let cropped = croppedPixelSize else { return nil }
        let longest = max(cropped.width, cropped.height)
        if longest <= edge { return cropped }
        let scale = Double(edge) / Double(longest)
        return (max(1, Int((Double(cropped.width) * scale).rounded())),
                max(1, Int((Double(cropped.height) * scale).rounded())))
    }

    func setStart(_ value: Double) {
        guard value.isFinite else { return }
        start = max(0, min(value, end - 0.05))
        if end - start > 30 { end = min(duration, start + 30) }
        playback.setRange(start: start, end: end)
        playback.seek(start)
    }

    func setEnd(_ value: Double) {
        guard value.isFinite else { return }
        end = min(duration, max(value, start + 0.05))
        if end - start > 30 { start = max(0, end - 30) }
        playback.setRange(start: start, end: end)
        playback.seek(max(start, end - 0.04))
    }

    func moveRange(to newStart: Double) {
        guard newStart.isFinite, duration > 0 else { return }
        let span = max(0.05, end - start)
        let clampedStart = max(0, min(duration - span, newStart))
        start = clampedStart
        end = min(duration, clampedStart + span)
        playback.setRange(start: start, end: end)
        playback.seek(start)
    }

    func setStartFromPlayhead() {
        let pos = playback.position
        if pos >= end - 0.05 {
            end = min(duration, pos + 0.5)
        }
        setStart(pos)
    }

    func setEndFromPlayhead() {
        let pos = playback.position
        if pos <= start + 0.05 {
            start = max(0, pos - 0.5)
        }
        setEnd(pos)
    }

    func resetCrop() {
        aspectLock = nil
        crop = CGRect(x: 0, y: 0, width: 1, height: 1)
    }

    func centeredCrop(_ aspect: Double?) {
        guard let info else { return }
        aspectLock = aspect
        guard let aspect else { crop = CGRect(x: 0, y: 0, width: 1, height: 1); return }
        let sourceAspect = Double(info.width) / Double(info.height)
        if sourceAspect > aspect {
            let width = aspect / sourceAspect
            crop = CGRect(x: (1 - width) / 2, y: 0, width: width, height: 1)
        } else {
            let height = sourceAspect / aspect
            crop = CGRect(x: 0, y: (1 - height) / 2, width: 1, height: height)
        }
    }

    private func invalidate() {
        revision += 1
        stale = result != nil
        failure = nil
        _ = renderTask?.cancel()
    }

    func chooseVideo() {
        guard !busy, !saving else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.movie, .video]
        panel.allowsOtherFileTypes = true
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.message = L10n.text(.creatorChooseHint)
        guard panel.runModal() == .OK, let url = panel.url else { return }
        open(url)
    }

    func open(_ url: URL) {
        guard !busy, !saving else { return }
        cancel()
        playback.clear()
        let previous = source
        let oldFallback = fallbackWork
        source = nil; info = nil; result = nil; thumbnails = []
        fallbackImage = nil; fallbackLease = nil
        stale = false; showingResult = false; failure = nil
        name = url.lastPathComponent
        busy = true; stage = "copying_video"
        work = Task { [weak self] in
            guard let self else { return }
            defer { self.busy = false; self.openTask = nil; self.previewTask = nil; self.work = nil; self.progressWork?.cancel() }
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            do {
                let library = try self.session.requireLibrary()
                await oldFallback?.value
                if let previous { _ = try await library.discardCreatorVideo(video: previous).awaitResult() }
                let task = try library.openCreatorVideo(path: url.path)
                self.openTask = task
                self.observe(task.progress())
                let video = try await withTaskCancellationHandler { try await task.awaitResult() } onCancel: { _ = task.cancel() }
                if Task.isCancelled {
                    _ = try? await library.discardCreatorVideo(video: video).awaitResult()
                    return
                }
                self.source = video; self.info = video.info()
                self.start = 0; self.end = min(5, self.duration)
                self.aspectLock = nil
                self.crop = CGRect(x: 0, y: 0, width: 1, height: 1)
                self.playback.load(path: video.path(), mime: video.info().mime, duration: self.duration, end: self.end)
                self.stage = "preparing_previews"
                for index in 0..<8 {
                    try Task.checkCancellation()
                    let time = UInt64(max(0, (self.duration - 0.001) * Double(index) / 7 * 1000))
                    let preview = try library.creatorPreview(video: video, timeMs: time)
                    self.previewTask = preview
                    let lease = try await withTaskCancellationHandler { try await preview.awaitResult() } onCancel: { _ = preview.cancel() }
                    try Task.checkCancellation()
                    if !self.thumbnails.contains(where: { $0.id == time }), let image = NSImage(contentsOfFile: try lease.metadata().path) {
                        self.thumbnails.append(CreatorThumbnail(id: time, lease: lease, image: image))
                        if self.fallbackImage == nil { self.fallbackImage = image }
                    }
                }
            } catch {
                if !Self.cancelled(error) { self.failure = Self.message(error) }
            }
        }
    }

    func generate() {
        guard canGenerate, let video = source else { return }
        playback.pause()
        busy = true; failure = nil; stage = "encoding_webp"
        let token = revision
        let edit = VideoEditOptions(
            startMs: UInt64((start * 1000).rounded()),
            endMs: UInt64((end * 1000).rounded()),
            cropX: crop.minX,
            cropY: crop.minY,
            cropWidth: crop.width,
            cropHeight: crop.height,
            maxEdge: UInt32(edge),
            fps: UInt32(fps),
            quality: UInt8(quality.rounded())
        )
        work = Task { [weak self] in
            guard let self else { return }
            defer { self.busy = false; self.renderTask = nil; self.work = nil; self.progressWork?.cancel() }
            do {
                let task = try self.session.requireLibrary().createVideoWebp(video: video, edit: edit)
                self.renderTask = task
                self.observe(task.progress())
                let output = try await withTaskCancellationHandler { try await task.awaitResult() } onCancel: { _ = task.cancel() }
                try Task.checkCancellation()
                guard token == self.revision else { return }
                self.result = output
                self.stale = false
                self.showingResult = true
            } catch {
                if !Self.cancelled(error) { self.failure = Self.message(error) }
            }
        }
    }

    func addToDock(collection: String?) {
        guard hasCurrentResult, let result, !saving else { return }
        saving = true
        Task {
            defer { saving = false }
            do {
                let library = try session.requireLibrary()
                let input = try await library.createImportInput().awaitResult()
                do {
                    let source = try result.artifact.metadata().path
                    let destination = input.path()
                    try await Task.detached {
                        let reader = try FileHandle(forReadingFrom: URL(fileURLWithPath: source))
                        let writer = try FileHandle(forWritingTo: URL(fileURLWithPath: destination))
                        defer { try? reader.close(); try? writer.close() }
                        while let data = try reader.read(upToCount: 64 * 1024), !data.isEmpty { try writer.write(contentsOf: data) }
                        try writer.synchronize()
                    }.value
                } catch {
                    _ = try? await library.discardImportInput(input: input).awaitResult()
                    throw error
                }
                let title = URL(fileURLWithPath: name).deletingPathExtension().lastPathComponent
                let outcome = try await library.importStaged(
                    input: input,
                    options: ImportOptions(originalName: "\(title).webp", title: title, collectionId: collection)
                ).awaitResult()
                if outcome.status == .restoreRequired { status.show(.restoreRequired) }
                else { status.show(.creatorAdded) }
                await session.refreshStatistics()
            } catch { failure = Self.message(error) }
        }
    }

    func save() async { guard let output, !stale else { return }; await perform(status) { try await actions.savePrepared(output) } }
    func share() async { guard let output, !stale else { return }; await perform(status) { try await actions.sharePrepared(output) } }

    func cancel() {
        work?.cancel(); _ = openTask?.cancel(); _ = renderTask?.cancel(); _ = previewTask?.cancel()
        progressWork?.cancel()
        fallbackWork?.cancel(); _ = fallbackTask?.cancel()
    }

    func shutdown() {
        cancel(); playback.clear()
        let previous = source
        let oldWork = work
        let oldFallback = fallbackWork
        source = nil; info = nil; thumbnails = []; result = nil
        if let previous, let library = session.library {
            Task { await oldWork?.value; await oldFallback?.value; _ = try? await library.discardCreatorVideo(video: previous).awaitResult() }
        }
    }

    /// AVFoundation cannot play every format supported by core. Scrubbing
    /// still requests accurate frames from the shared decoder for those files.
    func updateFallbackPreview() {
        guard !playback.available, !busy, let source else { return }
        fallbackWork?.cancel(); _ = fallbackTask?.cancel()
        let time = UInt64(max(0, min(duration - 0.001, playback.position)) * 1000)
        fallbackWork = Task { [weak self] in
            guard let self else { return }
            do {
                try await Task.sleep(for: .milliseconds(120))
                try Task.checkCancellation()
                let task = try self.session.requireLibrary().creatorPreview(video: source, timeMs: time)
                self.fallbackTask = task
                let lease = try await withTaskCancellationHandler { try await task.awaitResult() } onCancel: { _ = task.cancel() }
                try Task.checkCancellation()
                self.fallbackLease = lease
                self.fallbackImage = NSImage(contentsOfFile: try lease.metadata().path)
            } catch { if !Self.cancelled(error) { self.failure = Self.message(error) } }
        }
    }

    private func observe(_ progress: TaskProgressHandle) {
        progressWork?.cancel()
        progressWork = Task { [weak self] in
            while !Task.isCancelled, let snapshot = try? await progress.next() {
                self?.stage = snapshot.stage
            }
        }
    }

    private static func cancelled(_ error: Error) -> Bool {
        if error is CancellationError { return true }
        if case let BridgeError.Failure(code, _) = error, code == .cancelled { return true }
        return false
    }

    private static func message(_ error: Error) -> String {
        if case let BridgeError.Failure(code, detail) = error {
            if code == .unsupportedColorProfile { return L10n.text(.creatorSDROnly) }
            if code == .resourceLimit && detail.contains("decoder") { return L10n.text(.creatorInputResolution) }
            if code == .resourceLimit && detail.contains("2 GiB") { return L10n.text(.creatorInputSize) }
            if code == .resourceLimit { return L10n.text(.creatorResourceLimit) }
            if code == .invalidImage { return L10n.text(.creatorInvalidVideo) }
            if code == .unsupportedFormat { return L10n.text(.creatorUnsupported) }
            if code == .invalidInput { return L10n.text(.creatorInvalidSelection) }
            return describe(error)
        }
        return describe(error)
    }
}
