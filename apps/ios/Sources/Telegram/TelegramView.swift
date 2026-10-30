import MemeDockBridge
import SwiftUI

struct TelegramView: View {
    @Bindable var model: TelegramModel
    @State private var revealed = false
    @State private var leaving = false
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if model.metadata == nil {
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            Group {
                                if revealed {
                                    TextField("Bot Token", text: $model.token)
                                } else {
                                    SecureField("Bot Token", text: $model.token)
                                }
                            }.textInputAutocapitalization(.never).autocorrectionDisabled()
                            Button(revealed ? "Hide" : "Show", systemImage: revealed ? "eye.slash" : "eye") {
                                revealed.toggle()
                            }
                            .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                        }.textFieldStyle(.roundedBorder)
                        Text("Create a bot with Telegram’s @BotFather to get a token.").font(.caption).foregroundStyle(
                            .secondary)
                        HStack {
                            Toggle(
                                "Remember Token",
                                isOn: Binding(get: { model.rememberToken }, set: { model.setRemember($0) }))
                            Button("Forget Token", systemImage: "trash") { model.forget() }
                                .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                        }
                        TextField("Sticker pack name or t.me/addstickers link", text: $model.name)
                            .textInputAutocapitalization(.never).autocorrectionDisabled().textFieldStyle(.roundedBorder)
                        Button {
                            model.load()
                        } label: {
                            ZStack {
                                Text("View Stickers").opacity(model.isLoading ? 0 : 1).accessibilityHidden(
                                    model.isLoading)
                                HStack {
                                    Color.clear.frame(width: 20, height: 20).overlay {
                                        if model.isLoading { ProgressView() }
                                    }
                                    Text("Loading Stickers…")
                                }.opacity(model.isLoading ? 1 : 0).accessibilityHidden(!model.isLoading)
                            }.padding(.horizontal, 32).frame(maxWidth: .infinity, minHeight: 24)
                        }.buttonStyle(.borderedProminent).controlSize(.large)
                            .disabled(model.isBusy || model.token.isEmpty || model.name.isEmpty)
                            .overlay(alignment: .trailing) {
                                if model.isLoading {
                                    Button("Cancel", systemImage: "xmark") { model.cancel() }
                                        .labelStyle(.iconOnly).frame(width: 44, height: 44).padding(.trailing, 4)
                                        .disabled(model.isStopping)
                                }
                            }
                    }
                } else {
                    HStack {
                        Text("Choose Some Stickers").font(.headline)
                        Spacer()
                        Button("Change Pack", systemImage: "arrow.triangle.2.circlepath") { model.changePack() }
                            .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44).disabled(model.isBusy)
                    }
                    HStack {
                        Button("Select Not Imported", systemImage: "checkmark.circle") { model.selectNew() }
                        Button("Clear", systemImage: "xmark.circle") { model.selected = [] }.disabled(
                            model.selected.isEmpty)
                    }.labelStyle(.iconOnly).buttonStyle(.bordered).controlSize(.large).disabled(model.isBusy)
                    if let report = model.report, !model.isImporting {
                        let added = report.items.filter { $0.outcome == .created }.count
                        let reused = report.items.filter { $0.outcome == .reused }.count
                        let failed = report.items.filter { $0.outcome == .failed || $0.outcome == .pending }.count
                        Text("\(added) added · \(reused) existing · \(failed) failed").font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 4)], spacing: 4) {
                        ForEach(model.metadata?.stickers ?? [], id: \.id) { item in
                            TelegramTile(model: model, item: item)
                        }
                    }
                }
                if let error = model.error { Text(error).foregroundStyle(.red) }
            }.padding(16)
        }
        .navigationTitle(model.metadata?.title ?? String(localized: "Telegram Import"))
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(model.isBusy)
        .toolbar {
            if model.isBusy {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Back", systemImage: "chevron.left") { leaving = true }.labelStyle(.iconOnly)
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            if model.metadata != nil {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        ZStack(alignment: .leading) {
                            Text("\(model.selected.count) selected").opacity(model.isImporting ? 0 : 1)
                                .accessibilityHidden(model.isImporting)
                            let total = model.report?.items.count ?? 0
                            let done = model.report?.items.filter { $0.outcome != .pending }.count ?? 0
                            Text("Importing \(done) of \(total)").opacity(model.isImporting ? 1 : 0)
                                .accessibilityHidden(!model.isImporting)
                        }
                        ProgressView(
                            value: Double(model.report?.items.filter { $0.outcome != .pending }.count ?? 0),
                            total: Double(max(1, model.report?.items.count ?? 0))
                        )
                        .opacity(model.isImporting ? 1 : 0)
                        .accessibilityHidden(!model.isImporting)
                    }.font(.subheadline).frame(maxWidth: .infinity, alignment: .leading)
                    ZStack {
                        Button("Import Selected") { model.importSelected() }
                            .buttonStyle(.borderedProminent).controlSize(.large)
                            .frame(minHeight: 44).disabled(model.selected.isEmpty || model.isImporting)
                            .opacity(model.isImporting ? 0 : 1).accessibilityHidden(model.isImporting)
                            .allowsHitTesting(!model.isImporting)
                        Button("Cancel", systemImage: "xmark") { model.cancel() }
                            .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                            .disabled(model.isStopping || !model.isImporting)
                            .opacity(model.isImporting ? 1 : 0).accessibilityHidden(!model.isImporting)
                            .allowsHitTesting(model.isImporting)
                    }
                }.padding(.horizontal, 16).padding(.vertical, 8).background(.bar)
            }
        }
        .confirmationDialog("Leave Import?", isPresented: $leaving, titleVisibility: .visible) {
            Button("Stop and Leave", role: .destructive) {
                Task {
                    await model.stopAndWait()
                    dismiss()
                }
            }
        } message: {
            Text("Unfinished work will stop. Stickers already imported will stay.")
        }
        .task { await model.openPage() }
    }
}

private struct TelegramTile: View {
    let model: TelegramModel
    let item: TelegramSticker
    @State private var image: CGImage?
    @State private var retry = 0
    @State private var failed = false
    var body: some View {
        Button {
            model.toggle(item)
        } label: {
            Color.clear.aspectRatio(1, contentMode: .fit)
                .overlay {
                    ZStack {
                        if let image {
                            Image(decorative: image, scale: 1).resizable().scaledToFit().padding(4)
                        } else if failed {
                            Image(systemName: "photo").foregroundStyle(.secondary)
                        } else {
                            ProgressView()
                        }
                    }
                    .scaleEffect(model.selected.contains(item.id) ? 0.9 : 1)
                }
                .overlay(alignment: .topTrailing) {
                    if model.selected.contains(item.id) {
                        Image(systemName: "checkmark.circle.fill").foregroundStyle(Color.accentColor).padding(4)
                    }
                }
                .overlay(alignment: .bottomLeading) {
                    status.padding(4)
                }
                .background(
                    model.selected.contains(item.id) ? Color.accentColor.opacity(0.12) : .clear,
                    in: .rect(cornerRadius: 10)
                )
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(model.isBusy || (model.states[item.id] ?? item.state) == .restoreRequired)
        .accessibilityLabel(Text("Sticker \(item.emoji ?? "")"))
        .accessibilityAddTraits(model.selected.contains(item.id) ? .isSelected : [])
        .contextMenu { if failed { Button("Retry") { retry += 1 } } }
        .task(id: retry) {
            failed = false
            do {
                image = try await model.image(item)
                failed = image == nil
            } catch is CancellationError {} catch { failed = true }
        }
        .onDisappear { image = nil }
    }
    @ViewBuilder private var status: some View {
        let value = model.states[item.id] ?? item.state
        Group {
            switch value {
            case .imported: Text("Imported")
            case .restoreRequired: Text("In Trash")
            case .originalMissing: Text("Can Download Again")
            case .available:
                if item.format != .static { Text("Animated image") }
            }
        }
        .font(.caption2).padding(.horizontal, 5).padding(.vertical, 2)
        .foregroundStyle(.white).background(.black.opacity(0.5), in: .rect(cornerRadius: 4))
    }
}
