import SwiftUI
import MemeDockBridge

struct SyncSettings: View {
    @ObservedObject var coordinator: SyncCoordinator
    @State private var origin = ""
    @State private var token = ""
    @State private var changingToken = false
    @State private var confirmingDisconnect = false

    private var authorizing: Bool { changingToken || coordinator.needsAuthorization }

    var body: some View {
        SettingsForm {
            Section {
                if authorizing {
                    TextField(L10n.text(.syncServer), text: $origin)
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .disabled(coordinator.busy)
                    SecureField(L10n.text(.syncToken), text: $token)
                        .disabled(coordinator.busy)
                    Button(L10n.text(.syncConnect)) {
                        let credential = token
                        token = ""
                        Task { await coordinator.connect(origin: origin, token: credential); if coordinator.error == nil { changingToken = false } }
                    }
                    .disabled(coordinator.busy || origin.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || token.isEmpty)
                    .buttonStyle(.borderedProminent)
                } else {
                    LabeledContent(L10n.text(.syncServer), value: coordinator.state?.origin ?? "—")
                        .textSelection(.enabled)
                    if let device = coordinator.state?.deviceId {
                        LabeledContent(L10n.text(.deviceId)) { Text(device).font(MemeDockTypography.caption).textSelection(.enabled) }
                    }
                }
            } header: { SettingsHeader(.syncTab) }
            footer: { SettingsFooter(.syncTokenHint) }

            if let state = coordinator.state, state.connected, coordinator.hasCredential {
                Section {
                    LabeledContent(L10n.text(.syncStatus)) {
                        if coordinator.busy { ProgressView().controlSize(.small).accessibilityLabel(L10n.text(.syncRunning)) }
                        Text(L10n.text(coordinator.busy ? .syncRunning : state.paused ? .syncPaused : .syncReady))
                    }
                    LabeledContent(L10n.text(.syncPending), value: "\(state.pending)")
                    if state.blocked > 0 { LabeledContent(L10n.text(.syncBlocked), value: "\(state.blocked)") }
                    LabeledContent(L10n.text(.syncLastSuccess)) {
                        if let time = state.lastSuccessMs { Text(Date(timeIntervalSince1970: Double(time) / 1000), style: .relative) }
                        else { Text(L10n.text(.syncNever)) }
                    }
                    if !authorizing { HStack {
                        Button(L10n.text(state.paused ? .syncResume : .syncNow)) {
                            Task { if state.paused { await coordinator.setPaused(false) } else { await coordinator.synchronizeNow() } }
                        }.buttonStyle(.borderedProminent).disabled(coordinator.busy)
                        Spacer()
                        Menu {
                            Button(L10n.text(state.paused ? .syncResume : .syncPause)) { Task { await coordinator.setPaused(!state.paused) } }
                            Button(L10n.text(.syncReauthorize)) { Task { await coordinator.setPaused(true); changingToken = true } }
                            Divider()
                            Button(L10n.text(.syncDisconnect), role: .destructive) { confirmingDisconnect = true }
                        } label: { Image(systemName: "ellipsis") }
                        .menuStyle(.borderlessButton).fixedSize()
                        .accessibilityLabel(L10n.text(.more))
                    } }
                    if state.blocked > 0 { Text(L10n.text(.syncBlockedHint)).font(MemeDockTypography.caption) }
                } header: { SettingsHeader(.syncStatus) }
            }
            if let error = coordinator.error {
                Section { Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red) }
            }
            Section { SettingsFooter(.syncMergeHint) }
        }
        .onAppear { if origin.isEmpty { origin = coordinator.state?.origin ?? "http://127.0.0.1:8787" } }
        .onChange(of: coordinator.state?.origin) { _, value in if let value { origin = value } }
        .onDisappear { token = "" }
        .confirmationDialog(L10n.text(.syncDisconnectTitle), isPresented: $confirmingDisconnect) {
            Button(L10n.text(.syncDisconnect), role: .destructive) { Task { await coordinator.disconnect() } }
            Button(L10n.text(.cancel), role: .cancel) {}
        } message: { Text(L10n.text(.syncDisconnectHint)) }
    }
}
