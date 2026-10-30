import SwiftUI

struct SyncView: View {
    @Bindable var model: SyncModel
    @State private var origin = ""
    @State private var token = ""
    @State private var reauthorizing = false
    @State private var disconnecting = false
    var body: some View {
        Form {
            Section {
                if !model.hasCredential || reauthorizing || model.status?.error?.lowercased() == "unauthorized" {
                    TextField("Server Address", text: $origin).keyboardType(.URL).textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    SecureField("Device Token", text: $token).textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button("Connect and Sync") {
                        let credential = token
                        token = ""
                        Task {
                            await model.connect(origin: origin, token: credential)
                            if model.hasCredential { reauthorizing = false }
                        }
                    }.disabled(
                        origin.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || token.isEmpty || model.isBusy)
                } else {
                    LabeledContent("Server Address", value: model.status?.origin ?? "")
                    LabeledContent("Device ID", value: model.status?.deviceId ?? "")
                }
            } footer: {
                Text("Create a device in the web app, then enter its token here.")
            }
            if let status = model.status, status.connected {
                Section("Sync Status") {
                    LabeledContent("Sync Status", value: model.title)
                    LabeledContent("Pending", value: status.pending.formatted())
                    LabeledContent("Blocked", value: status.blocked.formatted())
                    LabeledContent("Last Synced") {
                        if let value = status.lastSuccessMs {
                            Text(Date(timeIntervalSince1970: Double(value) / 1000), style: .relative)
                        } else {
                            Text("Never Synced")
                        }
                    }
                }
            }
            Section {
                Toggle("Automatically Sync on Wi-Fi Only", isOn: $model.wifiOnly)
                if model.hasCredential {
                    Button(model.status?.paused == true ? "Resume Sync" : "Sync Now") {
                        Task {
                            if model.status?.paused == true {
                                await model.pause(false)
                            } else {
                                await model.synchronize()
                            }
                        }
                    }
                    if model.status?.paused != true { Button("Pause Sync") { Task { await model.pause(true) } } }
                    Button("Change Token") {
                        reauthorizing = true
                        Task { await model.pause(true) }
                    }
                    Button("Disconnect", role: .destructive) { disconnecting = true }
                }
                if model.isBusy { ProgressView("Syncing…") }
                if let error = model.error { Text(error).foregroundStyle(.red) }
                if let code = model.status?.error {
                    Text(syncError(code)).foregroundStyle(.red)
                }
            } footer: {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Automatic sync runs in the background when iOS schedules it. You can also sync manually.")
                    if let message = model.backgroundMessage { Text(message) }
                    Text(
                        "The first connection merges local and cloud stickers. Deletions also sync to your other devices."
                    )
                    if (model.status?.blocked ?? 0) > 0 {
                        Text("Some changes couldn’t sync. Open the web app to review conflicts.")
                    }
                }
            }
        }
        .navigationTitle("Sync").disabled(model.isBusy)
        .task { if origin.isEmpty { origin = model.status?.origin ?? "" } }
        .confirmationDialog("Disconnect?", isPresented: $disconnecting, titleVisibility: .visible) {
            Button("Disconnect", role: .destructive) { Task { await model.disconnect() } }
        } message: {
            Text("Sync will stop. Stickers already on this device will stay.")
        }
    }
    private func syncError(_ code: String) -> String {
        switch code.lowercased() {
        case "unauthorized": String(localized: "The device token is no longer valid. Enter it again.")
        case "network", "timeout": String(localized: "Unable to connect. Sync will retry later.")
        default: String(localized: "Sync failed. Check your connection and try again.")
        }
    }
}
