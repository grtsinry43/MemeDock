import SwiftUI

/// Right-hand inspector for Creator, styled to match `StickerDetailPane`:
/// aspect presets, output dimensions/FPS/quality, and a single primary action.
struct CreatorOutputSettings: View {
    @Environment(\.colorScheme) private var scheme
    @Bindable var model: CreatorModel

    private struct AspectOption: Identifiable {
        let id: String
        let label: String
        let aspect: Double?
        let isFull: Bool
    }

    private var aspectOptions: [AspectOption] {
        [
            AspectOption(id: "full", label: L10n.text(.creatorFullFrame), aspect: nil, isFull: true),
            AspectOption(id: "1:1", label: "1:1", aspect: 1.0, isFull: false),
            AspectOption(id: "4:3", label: "4:3", aspect: 4.0 / 3.0, isFull: false),
            AspectOption(id: "16:9", label: "16:9", aspect: 16.0 / 9.0, isFull: false),
            AspectOption(id: "9:16", label: "9:16", aspect: 9.0 / 16.0, isFull: false),
        ]
    }

    var body: some View {
        let palette = MemeDockPalette(scheme)
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: MemeDockGeometry.sectionGap) {
                    cropSection(palette)
                    outputSection(palette)
                }
                .padding(MemeDockGeometry.pageInset)
            }
            Divider()
                .overlay(palette.separator)
            footerAction(palette)
        }
        .background(palette.raised)
    }

    private func cropSection(_ palette: MemeDockPalette) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(L10n.text(.creatorCrop))
                    .font(MemeDockTypography.sectionLabel)
                    .foregroundStyle(palette.secondary)
                Spacer()
                if model.isCropped {
                    Button(L10n.text(.creatorResetCrop)) {
                        model.resetCrop()
                    }
                    .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                    .font(MemeDockTypography.caption)
                    .disabled(model.busy)
                }
            }

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 68), spacing: 6)], spacing: 6) {
                ForEach(aspectOptions) { option in
                    MemeDockChoiceChip(
                        title: option.label,
                        selected: isSelected(option)
                    ) {
                        model.centeredCrop(option.aspect)
                        if model.showingResult { model.showingResult = false }
                    }
                }
            }
            .disabled(model.busy)

            if let cropped = model.croppedPixelSize {
                HStack(spacing: 6) {
                    Text("\(cropped.width) × \(cropped.height) px")
                        .font(MemeDockTypography.caption)
                        .monospacedDigit()
                        .foregroundStyle(palette.secondary)
                    if model.aspectLock == nil && model.isCropped {
                        Text("· \(L10n.text(.creatorFreeCrop))")
                            .font(MemeDockTypography.caption)
                            .foregroundStyle(palette.secondary)
                    }
                }
            }
        }
    }

    private func outputSection(_ palette: MemeDockPalette) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(L10n.text(.creatorOutput))
                .font(MemeDockTypography.sectionLabel)
                .foregroundStyle(palette.secondary)

            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.text(.creatorLongestEdge))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.secondary)
                HStack(spacing: 6) {
                    ForEach([128, 256, 384, 512], id: \.self) { edge in
                        MemeDockChoiceChip(title: "\(edge)", selected: model.edge == edge) {
                            model.edge = edge
                        }
                    }
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.text(.creatorFrameRate))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.secondary)
                HStack(spacing: 6) {
                    ForEach([10, 15, 20, 24, 30], id: \.self) { fps in
                        MemeDockChoiceChip(title: "\(fps)", selected: model.fps == fps) {
                            model.fps = fps
                        }
                    }
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(L10n.text(.creatorQuality))
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(palette.secondary)
                    Spacer()
                    Text("\(Int(model.quality.rounded()))")
                        .font(MemeDockTypography.caption)
                        .fontWeight(.medium)
                        .monospacedDigit()
                        .foregroundStyle(palette.text)
                }
                Slider(value: $model.quality, in: 30...100, step: 1)
                    .tint(palette.text)
                    .accessibilityLabel(L10n.text(.creatorQuality))
            }

            if let estimated = model.estimatedOutputSize {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.up.left.and.arrow.down.right")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(palette.secondary)
                    Text("\(estimated.width) × \(estimated.height) · \(model.fps) FPS · \(String(format: "%.2fs", max(0, model.end - model.start)))")
                        .font(MemeDockTypography.caption)
                        .monospacedDigit()
                        .foregroundStyle(palette.secondary)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            }

            Text(L10n.text(.creatorWebPHint))
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .disabled(model.busy)
    }

    private func footerAction(_ palette: MemeDockPalette) -> some View {
        VStack(spacing: 8) {
            Button { model.generate() } label: {
                HStack(spacing: 8) {
                    if model.busy {
                        ProgressView()
                            .controlSize(.small)
                            .tint(palette.raised)
                        Text(stage)
                    } else {
                        Label(
                            L10n.text(model.stale ? .creatorRegenerate : .creatorGenerate),
                            systemImage: "sparkles"
                        )
                    }
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(MemeDockButtonStyle(kind: .primary))
            .disabled(!model.canGenerate)

            if model.busy {
                Button(L10n.text(.cancel)) { model.cancel() }
                    .buttonStyle(MemeDockButtonStyle(kind: .secondary))
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(MemeDockGeometry.pageInset)
    }

    private func isSelected(_ option: AspectOption) -> Bool {
        if option.isFull {
            return !model.isCropped
        }
        guard let target = option.aspect else { return false }
        if let lock = model.aspectLock {
            return abs(lock - target) < 0.001
        }
        guard let info = model.info, model.isCropped else { return false }
        let ratio = Double(info.width) * model.crop.width / (Double(info.height) * model.crop.height)
        return abs(ratio - target) < 0.005
    }

    private var stage: String {
        switch model.stage {
        case "copying_video", "inspecting_video": return L10n.text(.creatorReading)
        case "preparing_previews": return L10n.text(.creatorPreparingPreviews)
        case "validating_creation", "publishing_creation", "committing": return L10n.text(.creatorValidating)
        default: return L10n.text(.creatorGenerating)
        }
    }
}
