import AppKit
import SwiftUI

/// Window root. It routes on the session phase so a slow or failed library open
/// is visible instead of showing an empty grid.
struct RootView: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var session: LibrarySession
    @ObservedObject var status: StatusCenter
    /// Handed to the library window, which is the only view that can answer what
    /// the settings window asks it to show.
    let navigation: LibraryNavigation
    @StateObject private var model: LibraryModel
    /// Observed so a theme or language change re-renders this window; applying
    /// them is `Preferences`' own job.
    @StateObject private var preferences = Preferences.shared

    init(
        session: LibrarySession,
        status: StatusCenter,
        clipboard: ClipboardGuard,
        navigation: LibraryNavigation
    ) {
        _session = ObservedObject(wrappedValue: session)
        _status = ObservedObject(wrappedValue: status)
        self.navigation = navigation
        _model = StateObject(
            wrappedValue: LibraryModel(session: session, status: status, clipboard: clipboard)
        )
    }

    var body: some View {
        content
            .font(MemeDockTypography.body)
            .foregroundStyle(MemeDockPalette(scheme).text)
            .tint(MemeDockPalette(scheme).text)
            .buttonStyle(MemeDockButtonStyle())
            .background(MemeDockPalette(scheme).canvas)
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
            LibraryWindow(model: model, navigation: navigation)
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
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var status: StatusCenter

    var body: some View {
        Group {
            if let message = status.message {
                let palette = MemeDockPalette(scheme)
                HStack(spacing: 12) {
                    Image(systemName: status.isFailure ? "exclamationmark.circle" : "checkmark.circle")
                        .foregroundStyle(status.isFailure ? palette.error : palette.success)
                    Text(message)
                        .font(MemeDockTypography.body)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button { status.clear() } label: {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(.plain)
                    .help(L10n.text(.close))
                }
                .foregroundStyle(palette.onInverse)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .frame(maxWidth: 560)
                .background(palette.inverse, in: RoundedRectangle(cornerRadius: MemeDockGeometry.message))
                .shadow(color: .black.opacity(0.12), radius: 6, y: 3)
                .padding(.horizontal, 16)
                .padding(.bottom, 16)
                .transition(reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: status.message)
    }
}
