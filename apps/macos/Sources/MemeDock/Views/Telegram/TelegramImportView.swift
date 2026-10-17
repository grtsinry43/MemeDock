import MemeDockBridge
import SwiftUI

/// Telegram pack import. The sheet only shows what the current step needs: the
/// token while there is none to use, the pack field until a pack is loaded,
/// and then the pack's stickers with the import on the bottom bar. Nothing is
/// committed until the user asks for it, so the sheet is also where a pack is
/// reviewed and dropped.
struct TelegramImportView: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var coordinator: TelegramCoordinator
    @ObservedObject private var previews: TelegramPreviewStore

    @FocusState private var editing: Field?
    /// A saved token is folded into one line; this unfolds it for replacing.
    @State private var changingToken = false

    private enum Field { case token, pack }

    init(coordinator: TelegramCoordinator) {
        self.coordinator = coordinator
        _previews = ObservedObject(wrappedValue: coordinator.previews)
    }

    private var loaded: Bool {
        coordinator.metadata != nil
    }

    private var showsTokenField: Bool {
        !coordinator.tokenSaved || changingToken
    }

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 16) {
                header
                if let metadata = coordinator.metadata {
                    packSummary(metadata)
                } else {
                    tokenArea
                    packFields
                }
            }
            .padding(20)

            if let metadata = coordinator.metadata {
                Divider()
                selectionBar
                stickerGrid(metadata)
            }

            Divider()
            footer
        }
        .frame(width: 720)
        // Before a pack is loaded the sheet is only as tall as its fields; the
        // grid gets the room once there is something to show in it.
        .frame(minHeight: loaded ? 480 : nil, idealHeight: loaded ? 620 : nil)
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).text)
        .buttonStyle(MemeDockButtonStyle())
        .background(MemeDockPalette(scheme).canvas)
        .animation(reduceMotion ? nil : MemeDockMotion.panel, value: loaded)
        .animation(reduceMotion ? nil : MemeDockMotion.panel, value: showsTokenField)
        .task {
            await coordinator.start()
            // Whatever is missing is where typing starts.
            editing = coordinator.tokenSaved ? .pack : .token
        }
        // Closing with escape has to stop the work just like the close button.
        .onDisappear { coordinator.dismiss() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L10n.text(.importFromTelegram))
                .font(MemeDockTypography.sectionTitle)
            if !loaded {
                Text(L10n.text(.importFromTelegramHint))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
            }
        }
    }

    @ViewBuilder
    private var tokenArea: some View {
        if showsTokenField {
            VStack(alignment: .leading, spacing: 8) {
                SecureField(L10n.text(.telegramToken), text: $coordinator.token)
                    .textFieldStyle(MemeDockFieldStyle(focused: editing == .token))
                    .focused($editing, equals: .token)
                    .disabled(coordinator.isBusy)
                    .onSubmit { editing = .pack }
                HStack(spacing: 12) {
                    Toggle(L10n.text(.rememberToken), isOn: $coordinator.remember)
                    Spacer(minLength: 0)
                    Text(L10n.text(.telegramTokenHint))
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(MemeDockPalette(scheme).secondary)
                }
            }
            .transition(.opacity)
        } else {
            HStack(spacing: 8) {
                Image(systemName: "key")
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                Text(L10n.text(.useSavedToken))
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                Spacer(minLength: 0)
                Button(L10n.text(.change)) {
                    changingToken = true
                    editing = .token
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                .disabled(coordinator.isBusy)
            }
            .transition(.opacity)
        }
    }

    private var packFields: some View {
        HStack(spacing: 8) {
            TextField(L10n.text(.packPlaceholder), text: $coordinator.packQuery)
                .textFieldStyle(MemeDockFieldStyle(focused: editing == .pack))
                .focused($editing, equals: .pack)
                .disabled(coordinator.isBusy)
                .onSubmit { Task { await coordinator.loadPack() } }
            Button(L10n.text(.viewStickers)) {
                Task { await coordinator.loadPack() }
            }
            .disabled(coordinator.isBusy || coordinator.packQuery.isEmpty || coordinator.token.isEmpty)
        }
    }

    /// The fields have done their job once a pack is open, so they fold into
    /// the pack's name and the way back to them.
    private func packSummary(_ metadata: TelegramPackMetadata) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(metadata.title.isEmpty ? metadata.name : metadata.title)
                    .font(MemeDockTypography.rowTitle)
                    .lineLimit(1)
                Text(L10n.text(.stickerCount, metadata.stickers.count))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
            }
            Spacer(minLength: 0)
            Button(L10n.text(.changePack)) {
                coordinator.discardPack()
                editing = .pack
            }
            .disabled(coordinator.isBusy)
        }
        .transition(.opacity)
    }

    private var selectionBar: some View {
        HStack(spacing: 12) {
            let count = coordinator.selected.count
            Text(L10n.text(.selectedCount, count))
                .font(MemeDockTypography.body)
                .monospacedDigit()
                .foregroundStyle(MemeDockPalette(scheme).secondary)
                .contentTransition(reduceMotion ? .opacity : .numericText(value: Double(count)))
                .animation(reduceMotion ? nil : .snappy(duration: 0.25), value: count)
            Spacer()
            Button(L10n.text(.selectNew)) { coordinator.selectNew() }
                .disabled(coordinator.isBusy)
            Button(L10n.text(.clearSelection)) { coordinator.clearSelection() }
                .disabled(coordinator.isBusy || coordinator.selected.isEmpty)
        }
        .controlSize(.small)
        .padding(.horizontal, 20)
        .padding(.vertical, 8)
    }

    private func stickerGrid(_ metadata: TelegramPackMetadata) -> some View {
        ScrollView {
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 92, maximum: 120), spacing: 10)],
                spacing: 10
            ) {
                ForEach(metadata.stickers, id: \.id) { sticker in
                    cell(sticker)
                }
            }
            .padding(16)
        }
        .frame(maxHeight: .infinity)
        .transition(.opacity)
    }

    private func cell(_ sticker: TelegramSticker) -> some View {
        let selected = coordinator.isSelected(sticker.id)
        let selectable = coordinator.isSelectable(sticker)
        return Button {
            coordinator.toggle(sticker.id)
        } label: {
            VStack(spacing: 4) {
                ZStack {
                    RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                        .fill(selected ? MemeDockPalette(scheme).pressed : .clear)
                    if let image = previews.image(for: sticker.id) {
                        Image(nsImage: image)
                            .resizable()
                            .interpolation(.high)
                            .aspectRatio(contentMode: .fit)
                            .padding(6)
                    } else if previews.failed.contains(sticker.id) {
                        Image(systemName: "photo")
                            .foregroundStyle(.tertiary)
                    } else {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
                .frame(height: 88)
                .overlay {
                    RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                        .strokeBorder(selected ? MemeDockPalette(scheme).text.opacity(0.45) : .clear, lineWidth: 1)
                }
                .overlay(alignment: .topTrailing) {
                    Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(MemeDockPalette(scheme).secondary)
                        .padding(3)
                        .opacity(selectable ? 1 : 0.3)
                }

                Text(sticker.emoji ?? L10n.text(.stickers))
                    .font(MemeDockTypography.body)
                    .lineLimit(1)
                if let badge = badge(sticker) {
                    Text(badge)
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
        }
        .buttonStyle(.plain)
        .disabled(!selectable || coordinator.isBusy)
        .help(coordinator.itemErrors[sticker.id]?.message ?? sticker.emoji ?? sticker.id)
        .task {
            if sticker.hasPreview { previews.load(sticker.id) }
        }
    }

    /// What the sticker is now, or what the last import did to it. The outcome
    /// wins: a failure is more useful than "already in the library".
    private func badge(_ sticker: TelegramSticker) -> String? {
        switch coordinator.outcomes[sticker.id] {
        case .failed: return L10n.text(.importFailed)
        default: break
        }
        switch coordinator.states[sticker.id] {
        case .imported: return L10n.text(.alreadyImported)
        case .restoreRequired: return L10n.text(.inTrash)
        case .originalMissing: return L10n.text(.restoreRequired)
        case .available, .none: return nil
        }
    }

    private var footer: some View {
        HStack(spacing: 12) {
            if coordinator.phase == .importing {
                let counts = coordinator.progressCounts
                ProgressView(
                    value: Double(counts.done),
                    total: Double(max(counts.total, 1))
                )
                .frame(width: 160)
                Text("\(counts.done)/\(counts.total)")
                    .font(MemeDockTypography.body)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
                    .contentTransition(reduceMotion ? .opacity : .numericText(value: Double(counts.done)))
                    .animation(reduceMotion ? nil : .snappy(duration: 0.25), value: counts.done)
            } else if coordinator.phase == .loading {
                ProgressView()
                    .controlSize(.small)
                Text(L10n.text(.applying))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
            }

            if let summary = coordinator.summary {
                Text(summary)
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            if let message = coordinator.failure {
                Label(message, systemImage: "exclamationmark.triangle")
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            }

            Spacer()

            if coordinator.isBusy {
                Button(L10n.text(.cancel)) { coordinator.cancel() }
            }
            Button(L10n.text(.close)) {
                coordinator.dismiss()
                dismiss()
            }
            if loaded {
                Button(L10n.text(.importSelected)) {
                    Task { await coordinator.importSelected() }
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(MemeDockButtonStyle(kind: .primary))
                .disabled(coordinator.selected.isEmpty || coordinator.isBusy)
            }
        }
        .padding(16)
        .background(MemeDockPalette(scheme).canvas)
    }
}
