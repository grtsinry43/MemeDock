import Foundation
import ImageIO
import MemeDockBridge
import Observation

actor TelegramPreviewStore {
    private let cache = NSCache<NSString, CGImage>()
    private var epoch: UInt64 = 0
    init() {
        cache.countLimit = 80
        cache.totalCostLimit = 20 * 1024 * 1024
    }
    func clear() {
        epoch &+= 1
        cache.removeAllObjects()
    }
    func image(library: LibraryHandle, pack: TelegramPackHandle, name: String, id: String) async throws -> CGImage {
        let key = "\(name):\(id)" as NSString
        if let value = cache.object(forKey: key) { return value }
        let current = epoch
        let task = try library.telegramPreview(pack: pack, id: id)
        let path = try await withTaskCancellationHandler {
            try await task.awaitResult()
        } onCancel: {
            _ = task.cancel()
        }
        try Task.checkCancellation()
        guard current == epoch,
            let source = CGImageSourceCreateWithURL(
                URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
            let image = CGImageSourceCreateThumbnailAtIndex(
                source, 0,
                [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceThumbnailMaxPixelSize: 256,
                    kCGImageSourceShouldCacheImmediately: true,
                ] as CFDictionary)
        else { throw PreviewError.invalidImage }
        cache.setObject(image, forKey: key, cost: image.bytesPerRow * image.height)
        return image
    }
}

@MainActor @Observable
final class TelegramModel {
    var token = ""
    var name = ""
    var rememberToken = false
    var selected: Set<String> = []
    private(set) var metadata: TelegramPackMetadata?
    private(set) var report: TelegramImportReport?
    private(set) var states: [String: TelegramImportState] = [:]
    private(set) var isLoading = false
    private(set) var isImporting = false
    private(set) var isStopping = false
    private(set) var error: String?
    var isBusy: Bool { isLoading || isImporting }
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let feedback: AppFeedback
    @ObservationIgnored private let previews = TelegramPreviewStore()
    @ObservationIgnored private var pack: TelegramPackHandle?
    @ObservationIgnored private var loading: TelegramPackTask?
    @ObservationIgnored private var importing: TelegramImportTask?
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var credentialWork: Task<Void, Never>?

    init(library: LibraryHandle, feedback: AppFeedback) {
        self.library = library
        self.feedback = feedback
    }
    func openPage() async {
        cancel()
        await work?.value
        metadata = nil
        pack = nil
        report = nil
        selected = []
        states = [:]
        error = nil
        await previews.clear()
        do {
            if let value = try await SecureCredentials.shared.read("telegram") {
                token = value
                rememberToken = true
            }
        } catch { self.error = error.localizedDescription }
    }
    func setRemember(_ value: Bool) {
        rememberToken = value
        let previous = credentialWork
        let currentToken = token
        credentialWork = Task {
            await previous?.value
            do {
                if value && !currentToken.isEmpty {
                    try await SecureCredentials.shared.save(currentToken, account: "telegram")
                } else {
                    try await SecureCredentials.shared.remove("telegram")
                }
            } catch { self.error = error.localizedDescription }
        }
    }
    func forget() {
        token = ""
        setRemember(false)
    }
    func load() {
        guard !closed, !isBusy else { return }
        isLoading = true
        error = nil
        isStopping = false
        let credential = token
        let source = name
        work = Task {
            defer {
                isLoading = false
                loading = nil
                isStopping = false
            }
            do {
                await credentialWork?.value
                if rememberToken { try await SecureCredentials.shared.save(credential, account: "telegram") }
                let task = try library.telegramPack(token: credential, nameOrLink: source)
                loading = task
                let value = try await task.awaitResult()
                guard !closed, !isStopping else { return }
                await previews.clear()
                pack = value
                metadata = value.metadata()
                states = Dictionary(
                    (metadata?.stickers ?? []).map { ($0.id, $0.state) }, uniquingKeysWith: { first, _ in first })
                selected = []
                report = nil
            } catch { if !isStopping, !closed { self.error = BridgeErrors.telegramDescription(error) } }
        }
    }
    func selectNew() {
        selected = Set(
            (metadata?.stickers ?? []).filter {
                let state = states[$0.id] ?? $0.state
                return state == .available || state == .originalMissing
            }.map(\.id))
    }
    func toggle(_ item: TelegramSticker) {
        guard !isBusy, (states[item.id] ?? item.state) != .restoreRequired else { return }
        if !selected.insert(item.id).inserted { selected.remove(item.id) }
    }
    func changePack() {
        guard !isBusy else { return }
        metadata = nil
        pack = nil
        report = nil
        selected = []
    }
    func importSelected() {
        guard !closed, !isBusy, let pack, !selected.isEmpty else { return }
        isImporting = true
        isStopping = false
        error = nil
        let items = (metadata?.stickers ?? []).filter { selected.contains($0.id) }.map(\.id)
        work = Task {
            defer {
                isImporting = false
                isStopping = false
                importing = nil
            }
            do {
                let task = try library.importTelegram(pack: pack, selected: items)
                importing = task
                let polling = Task { [weak model = self] in
                    while !Task.isCancelled {
                        model?.report = task.snapshot()
                        do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
                    }
                }
                let result: TelegramImportReport
                do { result = try await task.awaitResult() } catch {
                    polling.cancel()
                    await polling.value
                    throw error
                }
                polling.cancel()
                await polling.value
                guard !closed else { return }
                report = result
                selected = []
                for item in result.items {
                    switch item.outcome {
                    case .created, .reused: states[item.id] = .imported
                    case .restoreRequired: states[item.id] = .restoreRequired
                    case .failed, .pending: selected.insert(item.id)
                    }
                }
                if let error = result.items.first(where: { $0.error != nil })?.error {
                    self.error = BridgeErrors.telegramDescription(BridgeError.Failure(code: error, detail: ""))
                }
            } catch { if !closed { self.error = BridgeErrors.telegramDescription(error) } }
        }
    }
    func image(_ item: TelegramSticker) async throws -> CGImage? {
        guard !closed, let pack, let metadata, item.hasPreview else { return nil }
        return try await previews.image(library: library, pack: pack, name: metadata.name, id: item.id)
    }
    func cancel() {
        isStopping = true
        _ = loading?.cancel()
        try? importing?.cancel()
    }
    func stopAndWait() async {
        cancel()
        await work?.value
    }
    func close() async {
        closed = true
        await stopAndWait()
        await credentialWork?.value
        await previews.clear()
    }
}
