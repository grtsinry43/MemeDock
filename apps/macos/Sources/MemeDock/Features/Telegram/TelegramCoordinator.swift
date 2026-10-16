import Combine
import Foundation
import MemeDockBridge

/// Imports a Telegram sticker pack: the bot token is read from the keychain,
/// the pack is looked up by name or link, and nothing is committed until the
/// user picks which stickers to bring in.
///
/// The core owns every network call and every decision about what a sticker
/// becomes; this type only sequences the three steps and reports their state.
@MainActor
final class TelegramCoordinator: ObservableObject {
    enum Phase: Equatable {
        case idle
        case loading
        case ready
        case importing
    }

    @Published var token = ""
    @Published var remember = false
    @Published var packQuery = ""

    @Published private(set) var phase: Phase = .idle
    @Published private(set) var metadata: TelegramPackMetadata?
    @Published private(set) var states: [String: TelegramImportState] = [:]
    @Published private(set) var outcomes: [String: TelegramImportOutcome] = [:]
    @Published private(set) var itemErrors: [String: ErrorCode] = [:]
    @Published private(set) var selected: Set<String> = []
    @Published private(set) var progress: TelegramImportReport?
    @Published private(set) var summary: String?
    @Published private(set) var failure: String?

    let previews: TelegramPreviewStore

    private let session: LibrarySession
    private let status: StatusCenter
    private let credentials = Credentials()
    private var pack: TelegramPackHandle?
    private var packTask: TelegramPackTask?
    private var importTask: TelegramImportTask?
    private var poller: Task<Void, Never>?
    private var cancelRequested = false
    private var loadedCredentials = false

    /// How often the import task's snapshot is read while it runs. The report
    /// arrives per sticker, so a fifth of a second is already smooth enough.
    private static let pollInterval: UInt64 = 150_000_000

    init(session: LibrarySession, status: StatusCenter) {
        self.session = session
        self.status = status
        previews = TelegramPreviewStore(session: session)
    }

    var isBusy: Bool {
        phase == .loading || phase == .importing
    }

    var stickers: [TelegramSticker] {
        metadata?.stickers ?? []
    }

    /// Items the core has already handled, against the number asked for.
    var progressCounts: (done: Int, total: Int) {
        guard let progress else { return (0, 0) }
        return (progress.items.count - progress.items.filter { $0.outcome == .pending }.count, progress.items.count)
    }

    // MARK: - Token

    func start() async {
        guard !loadedCredentials else { return }
        loadedCredentials = true
        do {
            guard let saved = try await credentials.load() else { return }
            token = saved
            remember = true
        } catch {
            failure = L10n.text(.tokenFailed)
        }
    }

    func forgetToken() async {
        do {
            try await credentials.clear()
            token = ""
            remember = false
        } catch {
            failure = L10n.text(.tokenFailed)
        }
    }

    // MARK: - Pack

    func loadPack() async {
        guard !isBusy else { return }
        let token = token.trimmingCharacters(in: .whitespacesAndNewlines)
        let query = packQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard Self.isTokenShaped(token) else {
            failure = L10n.text(.tokenInvalid)
            return
        }
        guard !query.isEmpty else { return }
        failure = nil
        summary = nil
        outcomes.removeAll()
        itemErrors.removeAll()
        phase = .loading
        cancelRequested = false
        defer {
            phase = metadata == nil ? .idle : .ready
            packTask = nil
        }
        do {
            let library = try session.requireLibrary()
            let task = try library.telegramPack(token: token, nameOrLink: query)
            packTask = task
            let handle = try await task.awaitResult()
            guard !cancelRequested else { return }
            pack = handle
            previews.use(pack: handle)
            let metadata = handle.metadata()
            self.metadata = metadata
            states = Dictionary(
                metadata.stickers.map { ($0.id, $0.state) },
                uniquingKeysWith: { _, last in last }
            )
            selected = []
            if remember {
                // Only a working token is worth keeping.
                try? await credentials.save(token)
            }
        } catch {
            failure = Self.message(for: error)
            pack = nil
            metadata = nil
            previews.use(pack: nil)
        }
    }

    /// Backs out of the loaded pack but keeps the token and the query, so
    /// another pack is one edit away.
    func discardPack() {
        cancel()
        pack = nil
        metadata = nil
        states.removeAll()
        outcomes.removeAll()
        itemErrors.removeAll()
        selected = []
        progress = nil
        summary = nil
        failure = nil
        previews.use(pack: nil)
        phase = .idle
    }

    // MARK: - Selection

    /// A sticker whose original is missing can be fetched again, and one that
    /// is already in the library comes back as reused; both are worth sending.
    /// One sitting in the trash is a restore, not an import.
    func isSelectable(_ sticker: TelegramSticker) -> Bool {
        states[sticker.id] != .restoreRequired
    }

    func isSelected(_ id: String) -> Bool {
        selected.contains(id)
    }

    func toggle(_ id: String) {
        guard phase != .importing else { return }
        if selected.contains(id) {
            selected.remove(id)
        } else {
            selected.insert(id)
        }
    }

    func selectNew() {
        guard phase != .importing else { return }
        selected = Set(
            stickers
                .filter { states[$0.id] == .available || states[$0.id] == .originalMissing }
                .map(\.id)
        )
    }

    func clearSelection() {
        selected = []
    }

    // MARK: - Import

    func importSelected() async {
        guard let pack, !selected.isEmpty, phase != .importing else { return }
        failure = nil
        summary = nil
        outcomes.removeAll()
        itemErrors.removeAll()
        phase = .importing
        cancelRequested = false
        let ids = selected.sorted()
        progress = nil
        do {
            let library = try session.requireLibrary()
            let task = try library.importTelegram(pack: pack, selected: ids)
            importTask = task
            startPolling(task)
            let report = try await task.awaitResult()
            poller?.cancel()
            poller = nil
            importTask = nil
            apply(report)
        } catch {
            poller?.cancel()
            poller = nil
            importTask = nil
            progress = nil
            failure = Self.message(for: error)
        }
        phase = metadata == nil ? .idle : .ready
    }

    func cancel() {
        cancelRequested = true
        switch phase {
        case .loading:
            _ = packTask?.cancel()
        case .importing:
            // The report is observed through the task's snapshot, so the cancel
            // reply carries nothing extra.
            try? importTask?.cancel()
        case .idle, .ready:
            break
        }
    }

    /// Closing the sheet stops the work; an import already committed stays in
    /// the library either way.
    func dismiss() {
        cancel()
        previews.use(pack: nil)
    }

    private func startPolling(_ task: TelegramImportTask) {
        poller?.cancel()
        poller = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.pollInterval)
                guard !Task.isCancelled, let self else { return }
                self.progress = task.snapshot()
            }
        }
    }

    private func apply(_ report: TelegramImportReport) {
        progress = report
        for item in report.items {
            outcomes[item.id] = item.outcome
            itemErrors[item.id] = item.error
            switch item.outcome {
            case .created, .reused:
                states[item.id] = .imported
            case .restoreRequired:
                states[item.id] = .restoreRequired
            case .pending, .failed:
                break
            }
            selected.remove(item.id)
        }
        let summary = Self.summary(for: report)
        self.summary = summary
        failure = report.items.first(where: { $0.outcome == .failed })?.error?.message
        status.show(summary, isFailure: report.items.contains { $0.outcome == .failed })
        // The core reports no per-sticker change for a Telegram import, so the
        // grid is told to rebuild instead of waiting for an event that never
        // arrives.
        session.announceReload()
        Task { [weak self] in await self?.session.refreshStatistics() }
    }

    // MARK: - Messages

    private static func summary(for report: TelegramImportReport) -> String {
        let items = report.items
        let added = items.filter { $0.outcome == .created }.count
        let reused = items.filter { $0.outcome == .reused }.count
        let failed = items.filter { $0.outcome == .failed }.count
        let pending = items.filter { $0.outcome == .restoreRequired }.count
        if report.stopped {
            return L10n.text(.importStopped, added + reused)
        }
        if failed > 0 {
            return L10n.text(.importDonePartial, added + reused, failed)
        }
        if pending > 0 {
            return L10n.text(.importPendingRestore, pending)
        }
        if reused > 0 {
            return L10n.text(.importDoneReused, added, reused)
        }
        return L10n.text(.importDone, added)
    }

    private static func message(for error: Error) -> String {
        guard let bridge = error as? BridgeError,
              case let .Failure(code, _) = bridge
        else { return describe(error) }
        switch code {
        case .unauthorized: return L10n.text(.tokenInvalid)
        case .notFound: return L10n.text(.packMissing)
        default: return describe(error)
        }
    }

    /// The core rejects a malformed token before any request, so the same shape
    /// is checked here to answer with the token message instead of a network
    /// round trip.
    private static func isTokenShaped(_ token: String) -> Bool {
        guard let (number, secret) = token.splitOnce(separator: ":") else { return false }
        return !number.isEmpty
            && number.allSatisfy(\.isASCII)
            && number.allSatisfy(\.isNumber)
            && (20...128).contains(secret.count)
            && secret.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_" || $0 == "-") }
    }
}

private extension String {
    func splitOnce(separator: Character) -> (String, String)? {
        guard let index = firstIndex(of: separator) else { return nil }
        return (String(self[..<index]), String(self[index...].dropFirst()))
    }
}
