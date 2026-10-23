import Foundation
import ImageIO
import MemeDockBridge

/// Bounded generation and decoding, with shared requests and individually cancellable readers.
/// Decoding occurs on this actor, never on the UI actor. NSCache's eviction limits are advisory.
actor ThumbnailStore {
    private struct Job {
        let key: ThumbnailKey
        let item: LibraryItem
        let force: Bool
        var readers: [UUID: CheckedContinuation<CGImage, any Error>]
        var worker: Task<Void, Never>?
        var core: ThumbnailTask?
    }

    private let library: LibraryHandle
    private let cache = NSCache<NSString, CGImage>()
    private var jobs: [UUID: Job] = [:]
    private var byKey: [ThumbnailKey: UUID] = [:]
    private var queue: [UUID] = []
    private var running = 0
    private var closed = false

    init(library: LibraryHandle) {
        self.library = library
        cache.countLimit = 96
        cache.totalCostLimit = 24 * 1024 * 1024
    }

    func image(for item: LibraryItem, pixels: Int, retry: Bool = false) async throws -> CGImage {
        try Task.checkCancellation()
        guard !closed else { throw CancellationError() }
        let key = ThumbnailKey(item, pixels: pixels)
        if !retry, let image = cache.object(forKey: key.cacheKey) { return image }
        let reader = UUID()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                if let id = byKey[key], jobs[id] != nil {
                    jobs[id]?.readers[reader] = continuation
                } else {
                    let id = UUID()
                    jobs[id] = Job(key: key, item: item, force: retry, readers: [reader: continuation])
                    byKey[key] = id
                    queue.append(id)
                }
                pump()
            }
        } onCancel: {
            Task { await self.cancel(reader, key: key) }
        }
    }

    func clearCache() { cache.removeAllObjects() }

    func shutdown() async {
        closed = true
        let workers = jobs.values.compactMap(\.worker)
        for id in Array(jobs.keys) {
            guard var job = jobs[id] else { continue }
            job.readers.values.forEach { $0.resume(throwing: CancellationError()) }
            job.readers = [:]
            _ = job.core?.cancel()
            job.worker?.cancel()
            jobs[id] = job
        }
        queue = []
        byKey = [:]
        for worker in workers { await worker.value }
        jobs = [:]
        running = 0
        cache.removeAllObjects()
    }

    private func cancel(_ reader: UUID, key: ThumbnailKey) {
        guard let id = byKey[key], var job = jobs[id],
              let continuation = job.readers.removeValue(forKey: reader) else { return }
        continuation.resume(throwing: CancellationError())
        jobs[id] = job
        if job.readers.isEmpty {
            byKey.removeValue(forKey: key)
            if job.worker == nil {
                jobs.removeValue(forKey: id)
                queue.removeAll { $0 == id }
            } else {
                _ = job.core?.cancel()
                job.worker?.cancel()
            }
        }
    }

    private func pump() {
        while !closed, running < 2, !queue.isEmpty {
            let id = queue.removeFirst()
            guard jobs[id] != nil else { continue }
            running += 1
            jobs[id]?.worker = Task { await self.perform(id) }
        }
    }

    private func perform(_ id: UUID) async {
        guard let job = jobs[id] else { return }
        let result: Result<CGImage, any Error>
        do {
            try Task.checkCancellation()
            let path: String
            if !job.force, let ready = job.item.resource?.thumbnailPath {
                path = ready
            } else {
                let task = try library.requestThumbnail(id: job.item.id, priority: .visible)
                jobs[id]?.core = task
                path = try await task.awaitResult().path
            }
            try Task.checkCancellation()
            let sourceOptions = [kCGImageSourceShouldCache: false] as CFDictionary
            guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, sourceOptions),
                  let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceThumbnailMaxPixelSize: job.key.pixels,
                    kCGImageSourceShouldCacheImmediately: true,
                  ] as CFDictionary) else {
                throw CocoaError(.fileReadCorruptFile)
            }
            result = .success(image)
        } catch { result = .failure(error) }
        finish(id, result: result)
    }

    private func finish(_ id: UUID, result: Result<CGImage, any Error>) {
        guard let job = jobs.removeValue(forKey: id) else { return }
        if byKey[job.key] == id { byKey.removeValue(forKey: job.key) }
        if !closed, !job.readers.isEmpty, case .success(let image) = result {
            cache.setObject(image, forKey: job.key.cacheKey, cost: image.bytesPerRow * image.height)
        }
        job.readers.values.forEach { $0.resume(with: result) }
        running -= 1
        pump()
    }
}
