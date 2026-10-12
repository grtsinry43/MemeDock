import AppKit
import Combine
import Foundation
import MemeDockBridge

/// Implements the clipboard protocol the core expects: pin the exported file,
/// publish it, then report the resulting reference. Foreign ownership is only
/// ever observed by dropping the local copy — it is never turned into a
/// `reconcile(nil)` acknowledgement, because that would claim ownership of
/// content the app did not write.
@MainActor
final class ClipboardGuard: ObservableObject {
    private struct Entry {
        let lease: ArtifactLeaseHandle
        let reference: String
    }

    private let session: LibrarySession
    private var entry: Entry?
    private var observedChange = -1
    private var timer: Timer?
    private var closing = false

    init(session: LibrarySession) {
        self.session = session
    }

    /// Publishes `write` on the general pasteboard and records the reference.
    /// A failed write aborts the pin instead of leaving it dangling.
    func install(lease: ArtifactLeaseHandle, write: (NSPasteboard) throws -> Void) async throws {
        guard !closing else { return }
        let library = try session.requireLibrary()
        let reference = try await library.protectClipboard(lease: lease).awaitResult()
        let pasteboard = NSPasteboard.general
        do {
            try write(pasteboard)
        } catch {
            _ = try? await library.abortClipboard(reference: reference).awaitResult()
            throw error
        }
        observedChange = pasteboard.changeCount
        _ = try? await library.reconcileClipboard(observed: reference).awaitResult()
        entry = Entry(lease: lease, reference: reference)
        startWatching()
    }

    func beginShutdown() {
        closing = true
        timer?.invalidate()
        timer = nil
        entry = nil
    }

    private func startWatching() {
        timer?.invalidate()
        let timer = Timer(timeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.check() }
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func check() {
        guard entry != nil else {
            timer?.invalidate()
            timer = nil
            return
        }
        guard NSPasteboard.general.changeCount != observedChange else { return }
        // Someone else copied something: stop pinning our file.
        timer?.invalidate()
        timer = nil
        entry = nil
    }
}
