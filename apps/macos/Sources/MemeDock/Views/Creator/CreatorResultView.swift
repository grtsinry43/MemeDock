import MemeDockBridge
import SwiftUI

/// Preview and export view for a generated animated WebP sticker.
struct CreatorResultView: View {
    @Environment(\.colorScheme) private var scheme
    @Bindable var model: CreatorModel
    let collectionId: String?
    @StateObject private var playback = AnimatedPlayback()

    private var path: String? { try? model.output?.metadata().path }

    var body: some View {
        let palette = MemeDockPalette(scheme)
        VStack(spacing: 16) {
            if model.stale {
                staleBanner(palette)
            }

            // Animated WebP preview card
            ZStack {
                RoundedRectangle(cornerRadius: 14)
                    .fill(palette.raised)
                    .overlay {
                        RoundedRectangle(cornerRadius: 14)
                            .strokeBorder(palette.separator, lineWidth: 1)
                    }

                if let frame = playback.frame {
                    Image(nsImage: frame)
                        .resizable()
                        .scaledToFit()
                        .padding(16)
                } else if playback.state == .broken || playback.state == .limit {
                    VStack(spacing: 8) {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: 24))
                            .foregroundStyle(palette.secondary)
                        Text(L10n.text(.failedToLoad))
                            .font(MemeDockTypography.caption)
                            .foregroundStyle(palette.secondary)
                    }
                }
            }
            .frame(maxWidth: .infinity, minHeight: 220, maxHeight: .infinity)

            // Measured WebP metadata pill
            if let result = model.result {
                HStack(spacing: 8) {
                    Text("WebP")
                        .font(MemeDockTypography.caption)
                        .fontWeight(.semibold)
                        .foregroundStyle(palette.text)
                    Text("·")
                        .foregroundStyle(palette.secondary.opacity(0.6))
                    Text("\(result.width) × \(result.height)")
                        .monospacedDigit()
                    Text("·")
                        .foregroundStyle(palette.secondary.opacity(0.6))
                    Text(String(format: "%.2f s", Double(result.durationMs) / 1000))
                        .monospacedDigit()
                    Text("·")
                        .foregroundStyle(palette.secondary.opacity(0.6))
                    Text("\(Int(result.frameRate)) FPS")
                        .monospacedDigit()
                    if let bytes = try? result.artifact.metadata().byteSize {
                        Text("·")
                            .foregroundStyle(palette.secondary.opacity(0.6))
                        Text(ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file))
                            .fontWeight(.medium)
                            .monospacedDigit()
                            .foregroundStyle(palette.text)
                    }
                }
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            }

            // Primary & secondary actions
            HStack(spacing: 10) {
                Button { model.addToDock(collection: collectionId) } label: {
                    HStack(spacing: 6) {
                        if model.saving {
                            ProgressView()
                                .controlSize(.small)
                                .tint(palette.raised)
                        } else {
                            Image(systemName: "plus.square.on.square")
                        }
                        Text(L10n.text(.creatorAddToDock))
                    }
                }
                .buttonStyle(MemeDockButtonStyle(kind: .primary))
                .disabled(!model.hasCurrentResult || model.saving)

                Button { Task { await model.save() } } label: {
                    Label(L10n.text(.saveAs), systemImage: "arrow.down.to.line")
                }
                .buttonStyle(MemeDockButtonStyle(kind: .secondary))
                .disabled(model.output == nil || model.stale || model.saving)

                Button { Task { await model.share() } } label: {
                    Label(L10n.text(.share), systemImage: "square.and.arrow.up")
                }
                .buttonStyle(MemeDockButtonStyle(kind: .secondary))
                .disabled(model.output == nil || model.stale || model.saving)
            }

            Text(L10n.text(.creatorResultHint))
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 520)
        }
        .task(id: path) {
            playback.load(path: path, animated: true, mime: "image/webp")
        }
        .onWindowVisibilityChange { playback.setWindowVisible($0) }
        .onDisappear { playback.stop() }
    }

    private func staleBanner(_ palette: MemeDockPalette) -> some View {
        HStack(spacing: 12) {
            Image(systemName: "exclamationmark.circle")
                .foregroundStyle(palette.star)
            Text(L10n.text(.creatorStale))
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.text)
            Spacer(minLength: 8)
            Button(L10n.text(.creatorRegenerate)) { model.generate() }
                .buttonStyle(MemeDockButtonStyle(kind: .primary))
                .disabled(!model.canGenerate)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
    }
}
