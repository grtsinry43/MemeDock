import SwiftUI

struct MenuBarShortcutLabel: View {
    let title: L10nKey
    let symbol: String
    @State private var hovering = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .resizable()
                .scaledToFit()
                .frame(width: 22, height: 22)
                .frame(height: 25)
            Text(L10n.text(title)).font(MemeDockTypography.caption)
                .multilineTextAlignment(.center).lineLimit(2).frame(height: 32, alignment: .top)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 10).padding(.horizontal, 4)
        .background(hovering ? MemeDockPalette(scheme).text.opacity(0.06) : .clear,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }
}

struct MenuBarShortcutStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.opacity(configuration.isPressed ? 0.55 : 1)
    }
}

struct MenuBarImportSources: View {
    enum Source: CaseIterable, Identifiable {
        case files, clipboard, telegram
        var id: Self { self }

        var title: L10nKey {
            switch self {
            case .files: return .menuImportFiles
            case .clipboard: return .menuImportClipboard
            case .telegram: return .menuImportTelegram
            }
        }

        var symbol: String {
            switch self {
            case .files: return "folder"
            case .clipboard: return "clipboard"
            case .telegram: return "paperplane"
            }
        }
    }

    let select: (Source) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Source.allCases) { source in
                Button { select(source) } label: {
                    Label(L10n.text(source.title), systemImage: source.symbol)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
            }
        }
        .font(MemeDockTypography.body)
        .padding(8)
        .frame(width: 220)
    }
}

struct MenuBarRecents: View {
    @ObservedObject var model: QuickPickerModel
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var session: LibrarySession
    let openLibrary: (String?) -> Void
    let dragChanged: (Bool, Bool) -> Void

    init(model: QuickPickerModel, openLibrary: @escaping (String?) -> Void, dragChanged: @escaping (Bool, Bool) -> Void) {
        self.model = model
        _browser = ObservedObject(wrappedValue: model.browser)
        _session = ObservedObject(wrappedValue: model.session)
        self.openLibrary = openLibrary
        self.dragChanged = dragChanged
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(L10n.text(.recentlyUsed)).font(MemeDockTypography.control)
                Spacer()
                Button(L10n.text(.viewAll)) { openLibrary(nil) }
                    .buttonStyle(.plain).font(MemeDockTypography.caption).foregroundStyle(.secondary)
            }
            if session.phase != .ready {
                Color.clear.frame(height: 100)
            } else if !model.isPresented || browser.items.isEmpty {
                VStack(spacing: 8) {
                    if browser.isLoading || !model.isPresented { ProgressView().controlSize(.small) }
                    else if let failure = browser.failure {
                        Text(failure).foregroundStyle(.secondary)
                        Button(L10n.text(.retry)) { browser.reload() }
                    } else { Text(L10n.text(.libraryEmpty)).foregroundStyle(.secondary) }
                }
                .font(MemeDockTypography.caption)
                .frame(maxWidth: .infinity).frame(height: 100)
            } else {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: model.columns), spacing: 8) {
                    ForEach(browser.items.prefix(8)) { item in
                        QuickPickerTile(item: item, model: model, selected: model.focusedId == item.id,
                                        openLibrary: openLibrary, dragChanged: dragChanged)
                    }
                }
            }
        }
    }
}

struct MenuBarActivity: View {
    @ObservedObject var session: LibrarySession
    @ObservedObject var imports: ImportCoordinator
    @ObservedObject var telegram: TelegramCoordinator
    @ObservedObject var sync: SyncCoordinator
    let openSync: () -> Void

    var body: some View {
        Group {
            if let progress = imports.progress {
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(L10n.text(.menuImportProgress, progress.index, progress.total))
                        Spacer()
                        Button(action: imports.cancel) { Image(systemName: "xmark") }
                            .buttonStyle(.plain).help(L10n.text(.importCancel))
                    }
                    ProgressView(value: Double(progress.index - 1), total: Double(max(1, progress.total)))
                        .progressViewStyle(.linear)
                }
            } else if telegram.phase == .importing {
                let counts = telegram.progressCounts
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(counts.total > 0 ? L10n.text(.menuImportProgress, counts.done, counts.total) : L10n.text(.importProgress))
                        Spacer()
                        Button(action: telegram.cancel) { Image(systemName: "xmark") }
                            .buttonStyle(.plain).help(L10n.text(.importCancel))
                    }
                    if counts.total > 0 {
                        ProgressView(value: Double(counts.done), total: Double(counts.total)).progressViewStyle(.linear)
                    } else { ProgressView().controlSize(.mini) }
                }
            } else if telegram.phase == .loading {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.mini)
                    Text(L10n.text(.loadingStickerPack))
                }
            } else if case let .failed(message) = session.phase {
                HStack(alignment: .top) {
                    Label(L10n.text(.libraryOpenFailed), systemImage: "exclamationmark.circle").help(message)
                    Spacer()
                    Button(L10n.text(.retry)) { Task { await session.start() } }
                }
            } else if session.phase != .ready {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.mini)
                    Text(L10n.text(.loadingLibrary))
                }
            } else if let state = sync.state, state.connected {
                Button(action: openSync) {
                    HStack(spacing: 6) {
                        if sync.busy { ProgressView().controlSize(.mini) }
                        else { Image(systemName: state.paused ? "pause.circle" : syncFailed ? "exclamationmark.circle" : "arrow.triangle.2.circlepath") }
                        Text(L10n.text(sync.busy ? .syncRunning : syncFailed ? .syncFailed : state.paused ? .syncPaused : state.pending > 0 || state.downloading > 0 || state.waiting > 0 ? .syncIncomplete : state.lastSuccessMs == nil ? .syncNever : .syncReady))
                        Spacer()
                        Image(systemName: "chevron.right").font(.system(size: 9))
                    }
                }
                .buttonStyle(.plain).help(L10n.text(.syncStatus))
            }
        }
        .font(MemeDockTypography.caption).foregroundStyle(.secondary)
    }

    private var syncFailed: Bool {
        sync.error != nil || sync.state?.error != nil || (sync.state?.blocked ?? 0) > 0 || !sync.hasCredential
    }
}

struct MenuBarFeedback: View {
    @ObservedObject var picker: QuickPickerModel
    @ObservedObject private var pickerStatus: StatusCenter
    @ObservedObject private var actions: StickerActions
    @ObservedObject var appStatus: StatusCenter
    init(picker: QuickPickerModel, appStatus: StatusCenter) {
        self.picker = picker
        _pickerStatus = ObservedObject(wrappedValue: picker.status)
        _actions = ObservedObject(wrappedValue: picker.actions)
        self.appStatus = appStatus
    }

    var body: some View {
        let source = pickerStatus.message != nil ? pickerStatus : appStatus
        Group {
            if picker.activity != nil || actions.busy {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.mini)
                    Text(L10n.text(picker.activity ?? .quickPickerPreparing))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .font(MemeDockTypography.caption).foregroundStyle(.secondary)
                .padding(10)
                .background(.regularMaterial, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            } else if let message = source.message {
                FeedbackBanner(message: message, isFailure: source.isFailure, dismiss: source.clear)
                    .frame(height: 64)
            }
        }
    }
}
