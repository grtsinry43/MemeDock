import SwiftUI

/// The ways in that need setting up. The Telegram token is the only one: a
/// file import needs nothing, so it stays on the library window.
struct ImportSettings: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var telegram: TelegramCoordinator
    let status: StatusCenter
    let openTelegram: () -> Void

    var body: some View {
        SettingsForm {
            Section {
                LabeledContent(L10n.text(.telegramToken)) {
                    HStack(spacing: 12) {
                        Text(L10n.text(telegram.tokenSaved ? .tokenSaved : .tokenNotSaved))
                            .foregroundStyle(MemeDockPalette(scheme).secondary)
                        if telegram.tokenSaved {
                            Button(L10n.text(.forget)) { forget() }
                                .disabled(telegram.isBusy)
                        }
                    }
                }
                LabeledContent {
                    Button(L10n.text(.open), action: openTelegram)
                } label: {
                    SettingsLabel(.importFromTelegram, detail: .importFromTelegramHint)
                }
            } header: {
                SettingsHeader(.telegram)
            } footer: {
                SettingsFooter(.telegramSettingsHint)
            }
        }
        .task { await telegram.start() }
    }

    private func forget() {
        Task {
            await telegram.forgetToken()
            if telegram.tokenSaved {
                status.show(L10n.text(.tokenFailed), isFailure: true)
            }
        }
    }
}
