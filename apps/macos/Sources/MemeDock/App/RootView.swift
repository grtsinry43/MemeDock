import AppKit
import SwiftUI

/// Window root. It routes on the session phase so a slow or failed library open
/// is visible instead of showing an empty grid.
struct RootView: View {
    @ObservedObject var session: LibrarySession
    @ObservedObject var status: StatusCenter
    @StateObject private var model: LibraryModel
    /// Observed so a theme or language change re-renders this window; applying
    /// them is `Preferences`' own job.
    @StateObject private var preferences = Preferences.shared

    init(session: LibrarySession, status: StatusCenter, clipboard: ClipboardGuard) {
        _session = ObservedObject(wrappedValue: session)
        _status = ObservedObject(wrappedValue: status)
        _model = StateObject(
            wrappedValue: LibraryModel(session: session, status: status, clipboard: clipboard)
        )
    }

    var body: some View {
        content
            .frame(minWidth: 720, minHeight: 480)
            .overlay(alignment: .bottom) { StatusBanner(status: status) }
            .task { await session.start() }
    }

    @ViewBuilder
    private var content: some View {
        switch session.phase {
        case .opening, .closed:
            OpeningView()
        case .ready:
            LibraryWindow(model: model)
        case let .failed(message):
            FailureView(message: message) {
                Task { await session.start() }
            }
        }
    }

}

private struct OpeningView: View {
    var body: some View {
        VStack(spacing: 12) {
            ProgressView()
            Text(L10n.text(.loading))
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct FailureView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 36))
                .foregroundStyle(.orange)
            Text(message)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button(L10n.text(.retry), action: retry)
                .keyboardShortcut(.defaultAction)
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Also shown in the settings window, which runs whole-library actions of its
/// own and needs the same feedback.
struct StatusBanner: View {
    @ObservedObject var status: StatusCenter

    var body: some View {
        if let message = status.message {
            Text(message)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(.regularMaterial, in: Capsule())
                .overlay(Capsule().strokeBorder(.separator))
                .foregroundStyle(status.isFailure ? Color.red : Color.primary)
                .padding(.bottom, 18)
                .transition(.opacity)
        }
    }
}
