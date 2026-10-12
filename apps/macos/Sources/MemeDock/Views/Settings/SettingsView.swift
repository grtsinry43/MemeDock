import AppKit
import SwiftUI

/// The settings window. It holds what is about the app rather than about the
/// sticker being browsed: appearance and language, what the library occupies
/// and where it lives, the maintenance actions, and the operations that reach
/// outside it - the trash, Telegram and backups.
struct SettingsView: View {
    @ObservedObject var session: LibrarySession
    @ObservedObject var status: StatusCenter
    /// The library window answers what this window asks of it; this window is
    /// what the handle below is for, since handing a command over means getting
    /// out of the way.
    let navigation: LibraryNavigation
    @ObservedObject private var preferences = Preferences.shared

    @StateObject private var telegram: TelegramCoordinator
    @StateObject private var backups: BackupCoordinator

    @State private var host = HostWindow()
    @State private var showTelegram = false
    @State private var showBackup = false
    @State private var working = false
    @State private var checkpoint: String?

    init(session: LibrarySession, status: StatusCenter, navigation: LibraryNavigation) {
        _session = ObservedObject(wrappedValue: session)
        _status = ObservedObject(wrappedValue: status)
        self.navigation = navigation
        _telegram = StateObject(wrappedValue: TelegramCoordinator(session: session, status: status))
        _backups = StateObject(wrappedValue: BackupCoordinator(session: session, status: status))
    }

    /// Everything below the general section acts on an open library.
    private var libraryReady: Bool {
        session.library != nil
    }

    var body: some View {
        Form {
            generalSection
            librarySection
            storageSection
            locationSection
            maintenanceSection
            telegramSection
            backupSection
        }
        .formStyle(.grouped)
        .frame(width: 540)
        .overlay(alignment: .bottom) { StatusBanner(status: status) }
        .background { HostWindowReader(host: host) }
        .toolbar {
            ToolbarItem {
                Button {
                    Task { await session.refreshStatistics() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help(L10n.text(.retry))
                .disabled(working || !libraryReady)
            }
        }
        .sheet(isPresented: $showTelegram) {
            TelegramImportView(coordinator: telegram)
                .presentationCornerRadius(MemeDockGeometry.dialog)
        }
        .sheet(isPresented: $showBackup) {
            BackupView(coordinator: backups)
                .presentationCornerRadius(MemeDockGeometry.dialog)
        }
        .task {
            guard libraryReady else { return }
            await session.refreshStatistics()
        }
    }

    private var generalSection: some View {
        Section(L10n.text(.general)) {
            Picker(L10n.text(.appearance), selection: $preferences.appearance) {
                ForEach(AppearanceMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            Picker(L10n.text(.language), selection: $preferences.language) {
                ForEach(LanguageMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
        }
    }

    private var librarySection: some View {
        Section(L10n.text(.library)) {
            LabeledContent(L10n.text(.libraryId)) {
                Text(session.identity?.libraryId ?? "—")
                    .textSelection(.enabled)
            }
            LabeledContent(L10n.text(.deviceId)) {
                Text(session.identity?.deviceId ?? "—")
                    .textSelection(.enabled)
            }
        }
    }

    private var storageSection: some View {
        Section(L10n.text(.storage)) {
            if let statistics = session.statistics {
                LabeledContent(L10n.text(.originalCount), value: "\(statistics.knownAssets)")
                LabeledContent(
                    L10n.text(.originalBytes),
                    value: Format.size(statistics.readyOriginalBytes)
                )
                // A thumbnail and a prepared share are both rebuildable cache, so
                // they are one line, the way Linux counts them.
                LabeledContent(
                    L10n.text(.cacheBytes),
                    value: Format.size(statistics.thumbnailBytes + statistics.temporaryShareBytes)
                )
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.text(.loading))
                        .foregroundStyle(.secondary)
                }
            }
            // The library's own way to its trash, next to what it holds. It is
            // the same move as the trash row in the library's sidebar.
            Button(L10n.text(.trash)) { openTrash() }
                .disabled(!libraryReady)
        }
    }

    private var locationSection: some View {
        Section(L10n.text(.dataDirectory)) {
            location(L10n.text(.dataDirectory), session.paths.data)
            location(L10n.text(.cacheDirectory), session.paths.cache)
            location(L10n.text(.exportDirectory), session.paths.export)
        }
    }

    private var maintenanceSection: some View {
        Section {
            Button(L10n.text(.createCheckpoint)) { createCheckpoint() }
                .disabled(working || !libraryReady)
            if let checkpoint {
                LabeledContent(L10n.text(.createCheckpoint)) {
                    HStack(spacing: 8) {
                        Text(URL(fileURLWithPath: checkpoint).lastPathComponent)
                            .textSelection(.enabled)
                        Button(L10n.text(.revealInFinder)) { reveal(checkpoint) }
                            .buttonStyle(.link)
                    }
                }
            }
            Button(L10n.text(.cleanExports)) { cleanExports() }
                .disabled(working || !libraryReady)
            if working {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.text(.applying))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var telegramSection: some View {
        Section(L10n.text(.telegram)) {
            Button(L10n.text(.telegram)) { showTelegram = true }
                .disabled(!libraryReady)
            Text(L10n.text(.telegramTokenHint))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
        }
    }

    private var backupSection: some View {
        Section(L10n.text(.backup)) {
            // Both directions are one file, so the sheet holds the two buttons
            // and this is only the way in.
            Button(L10n.text(.backup)) { showBackup = true }
                .disabled(!libraryReady)
            Text(L10n.text(.backupHint))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
        }
    }

    private func location(_ title: String, _ url: URL) -> some View {
        LabeledContent(title) {
            HStack(spacing: 8) {
                Text(url.path)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
                    .help(url.path)
                Button(L10n.text(.revealInFinder)) { reveal(url.path) }
                    .buttonStyle(.link)
            }
        }
    }

    private func createCheckpoint() {
        guard !working, libraryReady else { return }
        working = true
        Task {
            defer { working = false }
            switch await session.createCheckpoint() {
            case let .success(path):
                checkpoint = path
                status.show(.checkpointCreated)
            case let .failure(error):
                status.show(error)
            }
        }
    }

    private func cleanExports() {
        guard !working, libraryReady else { return }
        working = true
        Task {
            defer { working = false }
            switch await session.cleanExportArtifacts() {
            case .success:
                status.show(.exportsCleaned)
                await session.refreshStatistics()
            case let .failure(error):
                status.show(error)
            }
        }
    }

    /// Hands the library the move and gets out of the way, so what the user sees
    /// next is the trash itself rather than the window that offered it.
    private func openTrash() {
        navigation.show(.deleted)
        host.window?.close()
        navigation.bringForward()
    }

    private func reveal(_ path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }
}
