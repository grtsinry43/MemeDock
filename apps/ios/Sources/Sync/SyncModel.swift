import Foundation
import MemeDockBridge
import Network
import Observation

@MainActor @Observable
final class SyncModel {
    private(set) var status: SyncStatus?
    private(set) var hasCredential = false
    private(set) var isBusy = false
    private(set) var error: String?
    private(set) var backgroundMessage: String?
    var wifiOnly: Bool {
        didSet { UserDefaults.standard.set(wifiOnly, forKey: "sync.wifiOnly") }
    }
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let libraryID: String
    @ObservationIgnored private var operation: SyncStatusTask?
    @ObservationIgnored private var work: Task<Bool, Never>?
    @ObservationIgnored private var feed: Task<Void, Never>?
    @ObservationIgnored private var subscription: SyncSubscriptionHandle?
    @ObservationIgnored private var automatic: Task<Void, Never>?
    @ObservationIgnored private let monitor = NWPathMonitor()
    @ObservationIgnored private var onWiFi = false
    @ObservationIgnored private var initialPath: CheckedContinuation<Void, Never>?
    @ObservationIgnored private var closed = false
    @ObservationIgnored private var maintenance = false

    init(library: LibraryHandle) {
        self.library = library
        let identity = library.identity()
        libraryID = identity.libraryId
        wifiOnly = UserDefaults.standard.bool(forKey: "sync.wifiOnly")
    }

    func start() async {
        guard !closed else { return }
        isBusy = true
        defer { isBusy = false }
        await withCheckedContinuation { continuation in
            initialPath = continuation
            monitor.pathUpdateHandler = { [weak self] path in
                let wifi = path.status == .satisfied && path.usesInterfaceType(.wifi)
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    self.onWiFi = wifi
                    self.initialPath?.resume()
                    self.initialPath = nil
                }
            }
            monitor.start(queue: DispatchQueue(label: "MemeDock.Sync.Network"))
        }
        guard !closed else { return }
        do {
            let value = try await library.syncStatus().awaitResult()
            status = value
            if value.connected, let origin = value.origin {
                hasCredential = try await SecureCredentials.shared.read(account(for: origin)) != nil
            }
            let stream = try library.subscribeSync()
            subscription = stream
            feed = Task { [weak self] in
                do {
                    while !Task.isCancelled, let value = try await stream.next() {
                        guard let self, !self.closed else { return }
                        self.status = value
                    }
                } catch {
                    guard !Task.isCancelled, let self, !self.closed else { return }
                    self.error = BridgeErrors.syncDescription(error)
                }
            }
        } catch { self.error = BridgeErrors.syncDescription(error) }
    }

    var title: String {
        if isBusy { return String(localized: "Syncing…") }
        if !hasCredential { return String(localized: "Not Connected") }
        if error != nil || status?.error != nil { return String(localized: "Sync Failed") }
        if status?.paused == true { return String(localized: "Paused") }
        if (status?.pending ?? 0) > 0 || (status?.blocked ?? 0) > 0 { return String(localized: "Pending") }
        return String(localized: "Up to Date")
    }

    func connect(origin: String, token: String) async {
        _ = await run {
            let value = try await self.awaitTask(self.library.connectSync(origin: origin, token: token))
            self.status = value
            self.hasCredential = false
            guard let normalized = value.origin, value.connected else { throw CredentialError.unavailable }
            try await SecureCredentials.shared.save(token, account: self.account(for: normalized))
            self.hasCredential = true
            return value
        }
        scheduleBackground()
    }

    @discardableResult
    func synchronize(automatically: Bool = false) async -> Bool {
        if automatically && (wifiOnly && !onWiFi || status?.paused == true || maintenance) { return false }
        guard hasCredential, let origin = status?.origin else { return false }
        return await run {
            guard let token = try await SecureCredentials.shared.read(self.account(for: origin)) else {
                throw CredentialError.unavailable
            }
            return try await self.awaitTask(self.library.synchronize(token: token))
        }
    }

    func pause(_ paused: Bool) async {
        _ = await run { try await self.awaitTask(self.library.setSyncPaused(paused: paused)) }
        if !paused { await synchronize() }
        scheduleBackground()
    }

    func disconnect() async {
        _ = await run {
            let value = try await self.awaitTask(self.library.setSyncPaused(paused: true))
            if let origin = value.origin { try await SecureCredentials.shared.remove(self.account(for: origin)) }
            self.hasCredential = false
            return value
        }
        BackgroundSyncScheduler.cancel()
    }

    func setActive(_ active: Bool) {
        automatic?.cancel()
        guard active, !closed else { return }
        automatic = Task { [weak self] in
            while !Task.isCancelled {
                guard self != nil else { return }
                await self?.synchronize(automatically: true)
                let retry = self?.status?.retryAfterMs ?? 0
                do { try await Task.sleep(for: .milliseconds(max(30_000, min(retry, 300_000)))) } catch { return }
            }
        }
    }

    func cancel() { _ = operation?.cancel() }
    func scheduleBackground() {
        backgroundMessage = BackgroundSyncScheduler.schedule(enabled: hasCredential && status?.paused != true)
    }
    func beginMaintenance() async {
        maintenance = true
        cancel()
        _ = await work?.value
    }
    func endMaintenance() { maintenance = false }

    func close() async {
        closed = true
        automatic?.cancel()
        cancel()
        try? subscription?.unsubscribe()
        feed?.cancel()
        monitor.cancel()
        initialPath?.resume()
        initialPath = nil
        _ = await work?.value
        await feed?.value
    }

    private func awaitTask(_ value: SyncStatusTask) async throws -> SyncStatus {
        operation = value
        return try await value.awaitResult()
    }
    private func account(for origin: String) -> String { "sync:\(libraryID):\(origin)" }
    private func run(_ operation: @escaping @MainActor () async throws -> SyncStatus) async -> Bool {
        guard !closed, !isBusy, !maintenance else { return false }
        isBusy = true
        error = nil
        work = Task {
            defer {
                self.isBusy = false
                self.operation = nil
            }
            do {
                let value = try await operation()
                guard !self.closed else { return false }
                self.status = value
                return value.error == nil
            } catch {
                if !self.closed { self.error = BridgeErrors.syncDescription(error) }
                return false
            }
        }
        return await work?.value ?? false
    }
}
