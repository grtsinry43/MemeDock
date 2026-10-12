import MemeDockBridge
import SwiftUI

/// Telegram pack import: credentials at the top, the pack's stickers below, and
/// the import itself on the bottom bar. Nothing is committed until the user
/// asks for it, so the sheet is also where a pack is reviewed and dropped.
struct TelegramImportView: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var coordinator: TelegramCoordinator
    @ObservedObject private var previews: TelegramPreviewStore

    @Environment(\.dismiss) private var dismiss

    init(coordinator: TelegramCoordinator) {
        self.coordinator = coordinator
        _previews = ObservedObject(wrappedValue: coordinator.previews)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Three rows that never need scrolling, so the grid below keeps the
            // rest of the sheet whether a pack is loaded or not.
            VStack(alignment: .leading, spacing: 16) {
                header
                tokenFields
                packFields
            }
            .padding(16)

            if let metadata = coordinator.metadata {
                Divider()
                selectionBar
                stickerGrid(metadata)
            } else {
                Spacer(minLength: 0)
            }

            Divider()
            footer
        }
        .frame(width: 720)
        .frame(minHeight: 420, idealHeight: 620)
        .font(MemeDockTypography.body)
        .foregroundStyle(MemeDockPalette(scheme).text)
        .tint(MemeDockPalette(scheme).accent)
        .buttonStyle(MemeDockButtonStyle())
        .background(MemeDockPalette(scheme).canvas)
        .task { await coordinator.start() }
        // Closing with escape has to stop the work just like the close button.
        .onDisappear { coordinator.dismiss() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L10n.text(.telegram))
                .font(MemeDockTypography.sectionTitle)
            Text(L10n.text(.telegramTokenHint))
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
        }
    }

    private var tokenFields: some View {
        VStack(alignment: .leading, spacing: 8) {
            SecureField(L10n.text(.telegramToken), text: $coordinator.token)
                .textFieldStyle(MemeDockFieldStyle())
                .disabled(coordinator.isBusy)
            HStack(spacing: 12) {
                Toggle(L10n.text(.rememberToken), isOn: $coordinator.remember)
                Button(L10n.text(.clearToken)) {
                    Task { await coordinator.forgetToken() }
                }
                .disabled(coordinator.isBusy || coordinator.token.isEmpty)
            }
            .font(MemeDockTypography.body)
        }
    }

    private var packFields: some View {
        HStack(spacing: 8) {
            TextField(L10n.text(.stickerPack), text: $coordinator.packQuery)
                .textFieldStyle(MemeDockFieldStyle())
                .disabled(coordinator.isBusy)
                .onSubmit { Task { await coordinator.loadPack() } }
            Button(coordinator.metadata == nil ? L10n.text(.viewStickers) : L10n.text(.chooseStickers)) {
                Task { await coordinator.loadPack() }
            }
            .disabled(coordinator.isBusy || coordinator.packQuery.isEmpty || coordinator.token.isEmpty)
        }
    }

    private var selectionBar: some View {
        HStack(spacing: 12) {
            if let title = coordinator.metadata?.title, !title.isEmpty {
                Text(title)
                    .font(MemeDockTypography.rowTitle)
                    .lineLimit(1)
            }
            Spacer()
            if !coordinator.selected.isEmpty {
                Text(L10n.text(.selectedCount, coordinator.selected.count))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
            }
            Button(L10n.text(.selectNew)) { coordinator.selectNew() }
                .disabled(coordinator.isBusy)
            Button(L10n.text(.clearSelection)) { coordinator.clearSelection() }
                .disabled(coordinator.isBusy || coordinator.selected.isEmpty)
            Button(L10n.text(.changePack)) { coordinator.discardPack() }
                .disabled(coordinator.isBusy)
        }
        .controlSize(.small)
        .padding(.horizontal, 16)
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
                        .fill(MemeDockPalette(scheme).input.opacity(0.45))
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
                        .strokeBorder(selected ? MemeDockPalette(scheme).accent : .clear, lineWidth: 1.5)
                }
                .overlay(alignment: .topTrailing) {
                    Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(selected ? Color.accentColor : Color.secondary)
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
        .help(sticker.emoji ?? sticker.id)
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
            } else if coordinator.phase == .loading {
                ProgressView()
                    .controlSize(.small)
                Text(L10n.text(.applying))
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
            }

            if let message = coordinator.failure {
                Label(message, systemImage: "exclamationmark.triangle")
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            } else if let summary = coordinator.summary {
                Text(summary)
                    .font(MemeDockTypography.body)
                    .foregroundStyle(.secondary)
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
            Button(L10n.text(.importSelected)) {
                Task { await coordinator.importSelected() }
            }
            .keyboardShortcut(.defaultAction)
            .buttonStyle(MemeDockButtonStyle(kind: .primary))
            .disabled(coordinator.selected.isEmpty || coordinator.isBusy)
        }
        .padding(16)
        .background(MemeDockGlass())
    }
}
