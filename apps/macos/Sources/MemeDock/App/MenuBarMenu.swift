import AppKit
import SwiftUI

struct MenuBarMenu: View {
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var session: LibrarySession
    let navigation: LibraryNavigation
    @ObservedObject var integration: SystemIntegration
    @ObservedObject var imports: ImportCoordinator
    @ObservedObject var telegram: TelegramCoordinator
    @ObservedObject var sync: SyncCoordinator
    @ObservedObject var status: StatusCenter
    @ObservedObject private var preferences = Preferences.shared
    @State private var host = HostWindow()
    @State private var dragging = false
    @State private var dropTargeted = false
    @State private var showImportSources = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            header
            MenuBarActivity(session: session, imports: imports, telegram: telegram, sync: sync, openSync: { showSettings(.sync) })
            shortcuts
            MenuBarRecents(model: integration.menuPicker, openLibrary: showItem, dragChanged: dragChanged)
            footer
        }
        .padding(16)
        .frame(width: 400)
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).text)
        .overlay(alignment: .bottom) {
            MenuBarFeedback(picker: integration.menuPicker, appStatus: status)
                .padding(.horizontal, 16).padding(.bottom, 48)
        }
        .background { HostWindowReader(host: host, onResolve: integration.beginMenuBarPresentation) }
        .overlay {
            if dropTargeted {
                Label(L10n.text(.dropToImport), systemImage: "square.and.arrow.down")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
                    .allowsHitTesting(false)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            let files = urls.filter(\.isFileURL)
            guard session.phase == .ready, !imports.isRunning, !files.isEmpty else { return false }
            Task { await imports.importFiles(files, collectionId: nil) }
            return true
        } isTargeted: { targeted in
            dropTargeted = targeted && session.phase == .ready && !imports.isRunning
        }
        .onAppear { integration.beginMenuBarPresentation(in: host.window) }
        .onDisappear { suspendIfHidden() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { notification in
            guard let window = notification.object as? NSWindow, window === host.window else { return }
            integration.beginMenuBarPresentation(in: window)
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didChangeOcclusionStateNotification)) { notification in
            guard let window = notification.object as? NSWindow, window === host.window else { return }
            if window.isVisible { integration.beginMenuBarPresentation(in: window) }
            else { suspendIfHidden() }
        }
        .onExitCommand { integration.dismissMenuBar() }
    }

    private var header: some View {
        HStack {
            Text("MemeDock").font(MemeDockTypography.sectionTitle)
            Spacer()
            Menu {
                if sync.state?.connected == true {
                    Button(L10n.text(.syncNow)) { Task { await sync.synchronizeNow() } }
                        .disabled(sync.busy || sync.state?.paused == true || !sync.hasCredential)
                    Divider()
                }
                Button(L10n.text(.quitApp)) { NSApp.terminate(nil) }
            } label: {
                Image(systemName: "ellipsis").frame(width: 24, height: 24)
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .accessibilityLabel(L10n.text(.more))
        }
    }

    private var shortcuts: some View {
        HStack(alignment: .top, spacing: 4) {
            Button(action: integration.showPickerFromMenu) {
                MenuBarShortcutLabel(title: .chooseSticker, symbol: "face.smiling")
            }.buttonStyle(MenuBarShortcutStyle())
            Button { showImportSources = true } label: {
                MenuBarShortcutLabel(title: .importAction, symbol: "square.and.arrow.down")
            }
            .buttonStyle(MenuBarShortcutStyle())
            .disabled(session.phase != .ready || imports.isRunning)
            .popover(isPresented: $showImportSources, arrowEdge: .bottom) {
                MenuBarImportSources(select: importFrom)
                    .onExitCommand { showImportSources = false }
            }
            Button { showLibrary() } label: {
                MenuBarShortcutLabel(title: .library, symbol: "square.grid.2x2")
            }.buttonStyle(MenuBarShortcutStyle())
            Button { showSettings() } label: {
                MenuBarShortcutLabel(title: .settings, symbol: "gearshape")
            }.buttonStyle(MenuBarShortcutStyle())
        }
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: 10) {
            if integration.shortcutDescription == nil, !preferences.shortcutHintDismissed {
                HStack(spacing: 8) {
                    Button(L10n.text(.configureQuickShortcut)) { showSettings(.general) }.buttonStyle(.plain)
                    Spacer(minLength: 0)
                    Button { preferences.shortcutHintDismissed = true } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain).accessibilityLabel(L10n.text(.close))
                }
                .font(MemeDockTypography.caption).foregroundStyle(.secondary)
            } else if let shortcut = integration.shortcutDescription {
                HStack {
                    Text(L10n.text(.quickPicker))
                    Spacer()
                    Text(shortcut).monospaced()
                }
                .font(MemeDockTypography.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func suspendIfHidden() {
        guard !dragging, !integration.menuHandoffInProgress else { return }
        integration.dismissMenuBar()
    }

    private func importFrom(_ source: MenuBarImportSources.Source) {
        showImportSources = false
        Task { @MainActor in
            // Let the source popover close before presenting a file chooser or
            // handing the operation to a different window.
            await Task.yield()
            guard session.phase == .ready, !imports.isRunning else { return }
            switch source {
            case .files:
                integration.dismissMenuBar()
                NSApp.activate(ignoringOtherApps: true)
                imports.presentPicker(collectionId: nil)
            case .clipboard:
                await imports.importClipboard(collectionId: nil)
            case .telegram:
                showLibrary(.telegramImport)
            }
        }
    }

    private func dragChanged(_ active: Bool, _ completed: Bool) {
        dragging = active
        if !active, completed || host.window?.isVisible == false { integration.dismissMenuBar() }
    }

    private func showItem(_ id: String?) {
        showLibrary(id.map(LibraryNavigation.Target.sticker) ?? .section(.all))
    }

    private func showLibrary(_ target: LibraryNavigation.Target? = nil) {
        integration.dismissMenuBar()
        navigation.openLibrary = { openWindow(id: "library") }
        if let target { navigation.show(target) }
        navigation.bringForward()
    }

    private func showSettings(_ tab: SettingsTab? = nil) {
        integration.dismissMenuBar()
        integration.requestedSettingsTab = tab
        NSApp.activate(ignoringOtherApps: true)
        openSettings()
    }
}
