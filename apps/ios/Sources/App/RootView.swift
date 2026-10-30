import SwiftUI

struct RootView: View {
    let session: LibrarySession
    @State private var navigation = AppNavigation()
    @AppStorage("appearance") private var appearance: AppAppearance = .system
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Group {
            switch session.phase {
            case .ready:
                AppTabsView(navigation: navigation, session: session)
                    .overlay {
                        if let presentation = navigation.hero.presentation {
                            StickerHeroOverlay(hero: navigation.hero, presentation: presentation, session: session)
                                .id(presentation.route)
                        }
                    }
                    .disabled(session.isRestoring)
            case .idle, .opening, .closing:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                ContentUnavailableView {
                    Label("Library Unavailable", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(message)
                } actions: {
                    Button("Retry") { Task { await session.retry() } }
                }
            case .closed:
                ContentUnavailableView("Library Closed", systemImage: "externaldrive")
            }
        }
        .preferredColorScheme(appearance.colorScheme)
        .safeAreaInset(edge: .bottom) {
            if session.isRestoring {
                HStack(spacing: 12) {
                    ProgressView()
                    Text("Restoring…")
                    Spacer()
                    Button("Cancel") { session.backup?.cancel() }
                }.padding(16).background(.bar)
            }
        }
        .tint(
            Color(
                uiColor: UIColor { @Sendable traits in
                    traits.userInterfaceStyle == .dark
                        ? UIColor(red: 110 / 255, green: 168 / 255, blue: 1, alpha: 1)
                        : UIColor(red: 31 / 255, green: 111 / 255, blue: 235 / 255, alpha: 1)
                })
        )
        .overlay(alignment: .top) {
            FeedbackBanner(feedback: session.feedback) { action in
                switch action {
                case .viewTrash:
                    navigation.selected = .mine
                    navigation.minePath = [.trash]
                case .viewImported(let id):
                    if let browser = session.browser { Task { await navigation.showImported(id, browser: browser) } }
                case .viewImport:
                    navigation.showImportProgress()
                }
            }
        }
        .sheet(
            item: Binding(
                get: { session.outputs?.presentation },
                set: { session.outputs?.presentation = $0 }
            ), onDismiss: { Task { await session.outputs?.presentationDismissed() } }
        ) { value in
            if let output = session.outputs { SystemOutputView(value: value, output: output) }
        }
        .onChange(of: session.phase) { _, phase in
            if phase != .ready { navigation = AppNavigation() }
        }
        .onChange(of: session.restoreEpoch) { _, _ in
            navigation = AppNavigation()
            navigation.selected = .mine
            navigation.minePath = [.backup]
        }
        .onChange(of: scenePhase) { _, phase in
            session.sync?.setActive(phase == .active)
            if phase == .background { session.sync?.scheduleBackground() }
        }
        .onChange(of: session.phase) { _, phase in
            if phase == .ready { session.sync?.setActive(scenePhase == .active) }
        }
    }
}
