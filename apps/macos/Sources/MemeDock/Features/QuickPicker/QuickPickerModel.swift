import AppKit
import Combine
import MemeDockBridge

@MainActor
final class QuickPickerModel: ObservableObject {
    let session: LibrarySession
    let browser: StickerBrowser
    let thumbnails: ThumbnailStore
    let collections: CollectionCatalog
    let actions: StickerActions
    let status = StatusCenter()
    @Published var focusedId: String?
    @Published private(set) var presentation = 0
    @Published private(set) var activity: L10nKey?
    let columns: Int
    private let loadsCollections: Bool
    private var subscriptions: Set<AnyCancellable> = []
    private var visible = false
    private var output: Task<Void, Never>?
    var copied: (() -> Void)?
    var paste: ((Int) async throws -> Void)?

    init(session: LibrarySession, clipboard: ClipboardGuard, columns: Int = 5, pageSize: UInt32 = 80, loadsCollections: Bool = true) {
        self.session = session
        self.columns = columns
        self.loadsCollections = loadsCollections
        browser = StickerBrowser(session: session, pageSize: pageSize)
        thumbnails = ThumbnailStore(session: session, capacity: 100)
        collections = CollectionCatalog(session: session)
        actions = StickerActions(session: session, clipboard: clipboard, status: status)
        session.$phase.sink { [weak self] phase in
            guard let self else { return }
            if phase == .ready, self.visible { self.reload() }
            else if phase != .ready { self.browser.suspend(); self.thumbnails.clear() }
        }.store(in: &subscriptions)
        session.events.sink { [weak self] event in
            guard let self, self.visible else { return }
            switch event {
            case .reload:
                self.thumbnails.clear()
                self.reload()
            case .usage where self.browser.sort == .lastUsed:
                self.browser.reload()
            default:
                self.browser.apply(event)
                if self.loadsCollections { self.collections.apply(event) }
                if case let .thumbnail(id) = event { self.thumbnails.invalidate(id) }
            }
        }.store(in: &subscriptions)
    }

    var focusedItem: StickerItem? {
        browser.items.first { $0.id == focusedId }
    }

    var acceptsInput: Bool { visible && session.phase == .ready && !browser.isLoading }
    var isPresented: Bool { visible }

    func present() {
        guard !visible else { return }
        visible = true
        presentation += 1
        status.clear()
        focusedId = nil
        browser.searchText = ""
        browser.sort = .lastUsed
        browser.filter = .recentlyUsed
        if session.phase == .ready { reload() }
    }

    func dismiss() {
        visible = false
        presentation += 1
        browser.suspend()
        thumbnails.clear()
        actions.cancelPendingDrag()
    }

    func retry() {
        Task { await session.start() }
    }

    func select(_ filter: LibraryFilter) {
        focusedId = nil
        browser.sort = filter == .recentlyUsed ? .lastUsed : .recent
        browser.filter = filter
    }

    func move(_ direction: GridDirection) {
        let items = browser.items
        guard !items.isEmpty else { return }
        let current = items.firstIndex { $0.id == focusedId }
        let offset: Int
        switch direction {
        case .left: offset = -1
        case .right: offset = 1
        case .up: offset = -columns
        case .down: offset = columns
        }
        let index = current.map { min(max($0 + offset, 0), items.count - 1) } ?? 0
        focusedId = items[index].id
    }

    func copy(_ item: StickerItem, asFile: Bool = false) {
        outputImage(item, asFile: asFile, shouldPaste: false)
    }

    func activate(_ item: StickerItem) {
        outputImage(item, asFile: false, shouldPaste: true)
    }

    private func outputImage(_ item: StickerItem, asFile: Bool, shouldPaste: Bool) {
        guard acceptsInput, item.asset != nil, output == nil, !actions.busy else { return }
        focusedId = item.id
        let presentation = self.presentation
        output = Task { [weak self] in
            guard let self else { return }
            self.activity = .copyingImage
            self.status.clear()
            defer { self.output = nil; self.activity = nil }
            var copiedChangeCount: Int?
            do {
                let changeCount: Int
                if asFile {
                    try await self.actions.copyFile(item)
                    changeCount = NSPasteboard.general.changeCount
                } else { changeCount = try await self.actions.copyImage(item) }
                copiedChangeCount = changeCount
                guard self.visible, self.presentation == presentation else { return }
                if shouldPaste {
                    guard let paste = self.paste else { throw AppFailure(.pasteTargetMissing) }
                    self.activity = .pastingImage
                    try await paste(changeCount)
                }
                self.copied?()
            } catch {
                guard !(error is CancellationError), self.visible, self.presentation == presentation else { return }
                self.reportOutputFailure(error, copiedChangeCount: copiedChangeCount,
                                         currentChangeCount: NSPasteboard.general.changeCount)
            }
        }
    }

    func save(_ item: StickerItem) {
        guard acceptsInput, item.asset != nil, output == nil, !actions.busy else { return }
        output = Task { [weak self] in
            guard let self else { return }
            self.activity = .savingImage
            self.status.clear()
            defer { self.output = nil; self.activity = nil }
            do { try await self.actions.save(item) }
            catch { self.reportFailure(.saveFailed, error) }
        }
    }

    func recordDrag(_ item: StickerItem) {
        guard let library = session.library else { return }
        Task {
            await perform(status) { _ = try await library.recordUse(id: item.id, action: .shareLaunched).awaitResult() }
        }
    }

    func shutdown() {
        output?.cancel()
        actions.cancelOutput()
        dismiss()
    }

    func reportFailure(_ key: L10nKey, _ error: Error) {
        status.show("\(L10n.text(key))\n\(describe(error))", isFailure: true, persistent: true)
    }

    /// A refused paste does not undo a successful copy or dismiss the picker.
    /// Distinguish it from a changed clipboard and from failure before copying.
    func reportOutputFailure(_ error: Error, copiedChangeCount: Int?, currentChangeCount: Int) {
        let clipboardChanged = copiedChangeCount.map { currentChangeCount != $0 } ?? false
        let unsupported = (error as? AppFailure)?.key == .pasteUnsupportedInput
        if copiedChangeCount != nil, unsupported, !clipboardChanged {
            status.show("\(L10n.text(.imageCopied))\n\(describe(error))", persistent: true)
        } else {
            reportFailure(clipboardChanged ? .pasteCancelled : copiedChangeCount != nil ? .manualPasteRequired : .copyFailed, error)
        }
    }

    private func reload() {
        browser.reload()
        if loadsCollections { Task { await collections.load() } }
    }
}
