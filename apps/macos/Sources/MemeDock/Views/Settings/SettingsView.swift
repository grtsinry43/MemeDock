import AppKit
import SwiftUI

/// The pages of the settings window, in sidebar order.
enum SettingsTab: String, CaseIterable, Identifiable {
    case general
    case output
    case importing
    case backup
    case storage
    case about

    var id: String { rawValue }

    var title: L10nKey {
        switch self {
        case .general: return .general
        case .output: return .outputTab
        case .importing: return .importAction
        case .backup: return .backupTab
        case .storage: return .storage
        case .about: return .about
        }
    }

    var symbol: String {
        switch self {
        case .general: return "gearshape"
        case .output: return "doc.on.doc"
        case .importing: return "square.and.arrow.down"
        case .backup: return "clock.arrow.circlepath"
        case .storage: return "internaldrive"
        case .about: return "info.circle"
        }
    }

    /// Pages that act on the open library rather than on the app.
    var needsLibrary: Bool {
        switch self {
        case .general, .output: return false
        case .importing, .backup, .storage, .about: return true
        }
    }
}

/// The settings window: the pages on the left, the selected page on the
/// right. Everything here is about the app rather than about the sticker
/// being browsed, so it lives behind Command-, instead of taking room from
/// the grid.
struct SettingsView: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var session: LibrarySession
    @ObservedObject var status: StatusCenter
    /// The import page hands the Telegram sheet over to the library window
    /// through this, since that is where it is presented.
    let navigation: LibraryNavigation
    @ObservedObject var telegram: TelegramCoordinator
    @ObservedObject private var preferences = Preferences.shared

    @StateObject private var backups: BackupCoordinator

    @State private var tab: SettingsTab = .general
    @State private var host = HostWindow()
    @Namespace private var sidebarSelection

    init(
        session: LibrarySession,
        status: StatusCenter,
        navigation: LibraryNavigation,
        telegram: TelegramCoordinator
    ) {
        _session = ObservedObject(wrappedValue: session)
        _status = ObservedObject(wrappedValue: status)
        self.navigation = navigation
        _telegram = ObservedObject(wrappedValue: telegram)
        _backups = StateObject(wrappedValue: BackupCoordinator(session: session, status: status))
    }

    var body: some View {
        // The sidebar is the window's only navigation, so it cannot be put
        // away: a collapse request is answered by keeping both columns.
        NavigationSplitView(columnVisibility: .constant(.all)) {
            sidebar
                .navigationSplitViewColumnWidth(220)
                .toolbar(removing: .sidebarToggle)
        } detail: {
            page
                .toolbar(removing: .sidebarToggle)
        }
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).text)
        .frame(width: 800)
        .frame(minHeight: 480, idealHeight: 560, maxHeight: 800)
        .overlay(alignment: .bottom) { StatusBanner(status: status) }
        .background { HostWindowReader(host: host) }
    }

    // MARK: - Sidebar

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(SettingsTab.allCases) { item in
                row(item)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(MemeDockGlass(sidebar: true))
        .focusable()
        .focusEffectDisabled()
        .onMoveCommand(perform: move)
    }

    /// The same neutral pill the library sidebar uses. One shape is shared by
    /// every row, so changing page slides it over instead of swapping it.
    private func row(_ item: SettingsTab) -> some View {
        let palette = MemeDockPalette(scheme)
        let selected = tab == item
        return Button { select(item) } label: {
            HStack(spacing: 8) {
                Image(systemName: item.symbol)
                    .frame(width: 18)
                Text(L10n.text(item.title))
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(selected ? palette.raised : palette.text)
            .padding(.horizontal, 10)
            .frame(minHeight: 34)
            .background {
                if selected {
                    RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                        .fill(palette.text)
                        .matchedGeometryEffect(id: "selection", in: sidebarSelection)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private func select(_ item: SettingsTab) {
        guard item != tab else { return }
        withAnimation(reduceMotion ? nil : MemeDockMotion.panel) {
            tab = item
        }
    }

    private func move(_ direction: MoveCommandDirection) {
        let tabs = SettingsTab.allCases
        guard let index = tabs.firstIndex(of: tab) else { return }
        switch direction {
        case .up where index > 0: select(tabs[index - 1])
        case .down where index + 1 < tabs.count: select(tabs[index + 1])
        default: break
        }
    }

    // MARK: - Page

    private var page: some View {
        ZStack {
            content(tab)
                .id(tab)
                .transition(pageTransition)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(MemeDockPalette(scheme).canvas)
        .navigationTitle(L10n.text(tab.title))
    }

    /// The new page rises a little as it fades in while the old one fades
    /// out, so the change reads as movement without crossing the sidebar.
    private var pageTransition: AnyTransition {
        guard !reduceMotion else { return .opacity }
        return .asymmetric(
            insertion: .opacity.combined(with: .offset(y: 8)),
            removal: .opacity
        )
    }

    @ViewBuilder
    private func content(_ tab: SettingsTab) -> some View {
        if tab.needsLibrary, session.library == nil {
            unavailable
        } else {
            switch tab {
            case .general:
                GeneralSettings(preferences: preferences)
            case .output:
                OutputSettings(preferences: preferences)
            case .importing:
                ImportSettings(telegram: telegram, status: status, openTelegram: openTelegram)
            case .backup:
                BackupSettings(session: session, coordinator: backups, status: status)
            case .storage:
                StorageSettings(session: session, status: status)
            case .about:
                AboutSettings(session: session, status: status)
            }
        }
    }

    /// Hands the import to the library and gets out of the way, so what the
    /// user sees next is the sheet over the library rather than this window.
    private func openTelegram() {
        navigation.show(.telegramImport)
        host.window?.close()
        navigation.bringForward()
    }

    /// Shown while the library is opening, or in place of a page that has
    /// nothing to act on because it failed to open.
    @ViewBuilder
    private var unavailable: some View {
        Group {
            if case let .failed(message) = session.phase {
                Label(message, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
            } else {
                SettingsProgress(.loading)
            }
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
