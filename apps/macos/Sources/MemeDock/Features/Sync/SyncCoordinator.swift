import AppKit
import Combine
import Foundation
import MemeDockBridge

/// macOS owns credentials and wake/quit behavior; Rust owns persisted sync work.
@MainActor
final class SyncCoordinator: ObservableObject {
    @Published private(set) var state: SyncStatus?
    @Published private(set) var busy = false
    @Published private(set) var hasCredential = false
    @Published private(set) var error: String?

    private weak var session: LibrarySession?
    private var observers = Set<AnyCancellable>()
    private var loop: Task<Void, Never>?
    private var feed: Task<Void, Never>?
    private var subscription: SyncSubscriptionHandle?
    private var operation: SyncStatusTask?
    private var generation: UInt64 = 0
    private var workVersion: UInt64 = 0

    init(session: LibrarySession) {
        self.session = session
        session.$phase.removeDuplicates().sink { [weak self] phase in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if phase == .ready { await self.restore() }
                else { await self.shutdown() }
            }
        }.store(in: &observers)
        NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.willSleepNotification)
            .sink { [weak self] _ in Task { @MainActor [weak self] in await self?.stop() } }
            .store(in: &observers)
        NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)
            .sink { [weak self] _ in Task { @MainActor [weak self] in await self?.resumeLoop() } }
            .store(in: &observers)
        NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in Task { @MainActor [weak self] in
                guard let self, !self.busy else { return }
                await self.resumeLoop()
            } }.store(in: &observers)
    }

    var needsAuthorization: Bool { !hasCredential || state?.error == "unauthorized" }

    private func credentials(origin: String) throws -> Credentials {
        guard let id = session?.identity?.libraryId else { throw AppFailure(.errorClosed) }
        return Credentials(account: "sync:\(id):\(origin)", label: "MemeDock Sync")
    }

    func restore() async {
        await stop()
        feed?.cancel()
        _ = try? subscription?.unsubscribe()
        generation &+= 1
        let version = generation
        guard let library = session?.library else { return }
        do {
            state = try await library.syncStatus().awaitResult()
            guard generation == version, session?.phase == .ready else { return }
            if let origin = state?.origin { hasCredential = try await credentials(origin: origin).load() != nil }
            let subscription = try library.subscribeSync()
            self.subscription = subscription
            feed = Task { [weak self] in
                do {
                    while !Task.isCancelled, let value = try await subscription.next() {
                        guard let self, self.generation == version else { return }
                        self.state = value
                    }
                } catch { /* A closed library ends this observation. */ }
            }
            await resumeLoop()
        } catch { self.error = describe(error) }
    }

    func connect(origin: String, token: String) async {
        guard let library = session?.library else { return }
        let version = await stop()
        guard workVersion == version else { return }
        busy = true
        error = nil
        defer { if workVersion == version { busy = false } }
        do {
            let task = try library.connectSync(origin: origin.trimmingCharacters(in: .whitespacesAndNewlines), token: token)
            operation = task
            let value = try await task.awaitResult()
            guard workVersion == version, session?.phase == .ready else { return }
            operation = nil
            guard let origin = value.origin else { throw AppFailure(.errorCorruptData) }
            do { try await credentials(origin: origin).save(token) }
            catch {
                guard workVersion == version else { return }
                state = try await library.setSyncPaused(paused: true).awaitResult()
                throw AppFailure(.syncKeychainFailed)
            }
            guard workVersion == version, session?.phase == .ready else { return }
            state = value
            hasCredential = true
            startLoop(token: token)
        } catch { if workVersion == version { operation = nil; self.error = describe(error) } }
    }

    func synchronizeNow() async { await resumeLoop() }

    func setPaused(_ paused: Bool) async {
        guard let library = session?.library else { return }
        let version = await stop()
        guard workVersion == version else { return }
        do {
            let value = try await library.setSyncPaused(paused: paused).awaitResult()
            guard workVersion == version, session?.phase == .ready else { return }
            state = value
            error = nil
            if !paused { await resumeLoop() }
        } catch { if workVersion == version { self.error = describe(error) } }
    }

    func disconnect() async {
        guard let library = session?.library else { return }
        let version = await stop()
        guard workVersion == version else { return }
        do {
            let value = try await library.setSyncPaused(paused: true).awaitResult()
            guard workVersion == version, session?.phase == .ready else { return }
            state = value
            if let origin = state?.origin { try await credentials(origin: origin).clear() }
            guard workVersion == version else { return }
            hasCredential = false
            error = nil
        } catch { if workVersion == version { self.error = describe(error) } }
    }

    private func resumeLoop() async {
        guard let state, state.connected, !state.paused, !busy, session?.phase == .ready,
              state.error != "unauthorized", let origin = state.origin else { return }
        let version = await stop()
        guard workVersion == version else { return }
        do {
            guard let token = try await credentials(origin: origin).load() else {
                hasCredential = false
                return
            }
            guard workVersion == version, session?.phase == .ready else { return }
            hasCredential = true
            startLoop(token: token)
        } catch { if workVersion == version { self.error = L10n.text(.syncKeychainFailed) } }
    }

    private func startLoop(token: String) {
        guard loop == nil else { return }
        let version = generation
        loop = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, self.generation == version, let library = self.session?.library else { return }
                self.busy = true
                var delay: UInt64 = 30_000
                do {
                    let task = try library.synchronize(token: token)
                    self.operation = task
                    let state = try await task.awaitResult()
                    guard !Task.isCancelled, self.generation == version else { self.busy = false; return }
                    self.state = state
                    self.error = nil
                    delay = state.retryAfterMs
                } catch {
                    if Task.isCancelled { self.busy = false; return }
                    self.error = describe(error)
                    self.state = try? await library.syncStatus().awaitResult()
                    if self.state?.error == "unauthorized" { self.busy = false; self.operation = nil; self.loop = nil; return }
                }
                self.operation = nil
                self.busy = false
                do { try await Task.sleep(nanoseconds: max(delay, 250) * 1_000_000) }
                catch { return }
            }
        }
    }

    @discardableResult
    private func stop() async -> UInt64 {
        workVersion &+= 1
        let version = workVersion
        let current = loop
        loop = nil
        current?.cancel()
        _ = operation?.cancel()
        await current?.value
        if workVersion == version { operation = nil; busy = false }
        return version
    }

    func shutdown() async {
        generation &+= 1
        await stop()
        feed?.cancel()
        feed = nil
        _ = try? subscription?.unsubscribe()
        subscription = nil
        state = nil
        hasCredential = false
    }
}
