import SwiftUI
import UniformTypeIdentifiers

struct CreatorWorkspace: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Bindable var model: CreatorModel
    let collectionId: String?
    @State private var dropTargeted = false

    var body: some View {
        let palette = MemeDockPalette(scheme)
        GeometryReader { geometry in
            if model.source == nil {
                emptyState(palette)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if geometry.size.width >= 760 {
                HStack(spacing: 0) {
                    stage(palette)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    Rectangle()
                        .fill(palette.separator)
                        .frame(width: 1)
                    CreatorOutputSettings(model: model)
                        .frame(width: 268)
                }
            } else {
                ScrollView {
                    VStack(spacing: 0) {
                        stage(palette)
                            .frame(minHeight: 460)
                        Rectangle()
                            .fill(palette.separator)
                            .frame(height: 1)
                        CreatorOutputSettings(model: model)
                    }
                }
            }
        }
        .background(palette.canvas)
        .onDrop(of: [.fileURL], isTargeted: $dropTargeted, perform: handleDrop)
        .focusable()
        .focusEffectDisabled()
        .onKeyPress(.space) {
            guard model.source != nil, !model.showingResult, !model.busy else { return .ignored }
            model.playback.toggle()
            return .handled
        }
        .onKeyPress(.leftArrow) {
            guard model.source != nil, !model.showingResult, !model.busy else { return .ignored }
            model.playback.step(by: -0.1)
            return .handled
        }
        .onKeyPress(.rightArrow) {
            guard model.source != nil, !model.showingResult, !model.busy else { return .ignored }
            model.playback.step(by: 0.1)
            return .handled
        }
        .onKeyPress(characters: CharacterSet(charactersIn: "[]")) { press in
            guard model.source != nil, !model.showingResult, !model.busy else { return .ignored }
            if press.characters == "[" {
                model.setStartFromPlayhead()
                return .handled
            } else if press.characters == "]" {
                model.setEndFromPlayhead()
                return .handled
            }
            return .ignored
        }
        .onWindowVisibilityChange { model.playback.setWindowVisible($0) }
        .onDisappear { model.playback.pause() }
        .onChange(of: model.playback.position) { _, _ in model.updateFallbackPreview() }
    }

    private func emptyState(_ palette: MemeDockPalette) -> some View {
        VStack(spacing: 20) {
            VStack(spacing: 16) {
                Image(systemName: dropTargeted ? "arrow.down.doc" : "film.stack")
                    .font(.system(size: 40, weight: .light))
                    .foregroundStyle(dropTargeted ? palette.text : palette.secondary)
                VStack(spacing: 6) {
                    Text(L10n.text(.creatorTitle))
                        .font(MemeDockTypography.sectionTitle)
                        .foregroundStyle(palette.text)
                    Text(L10n.text(.creatorEmptyHint))
                        .font(MemeDockTypography.body)
                        .foregroundStyle(palette.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 420)
                }
                if model.busy {
                    HStack(spacing: 10) {
                        ProgressView()
                            .controlSize(.small)
                        Text(L10n.text(.creatorReading))
                            .font(MemeDockTypography.body)
                            .foregroundStyle(palette.secondary)
                    }
                    Button(L10n.text(.cancel)) { model.cancel() }
                        .buttonStyle(MemeDockButtonStyle(kind: .secondary))
                } else {
                    Button { model.chooseVideo() } label: {
                        Label(L10n.text(.creatorChoose), systemImage: "folder")
                    }
                    .buttonStyle(MemeDockButtonStyle(kind: .primary))

                    Text(L10n.text(.creatorDropHint))
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(palette.secondary)
                }
            }
            .padding(.horizontal, 40)
            .padding(.vertical, 36)
            .frame(maxWidth: 520)
            .background(
                dropTargeted ? palette.input : palette.raised,
                in: RoundedRectangle(cornerRadius: MemeDockGeometry.group)
            )
            .overlay {
                RoundedRectangle(cornerRadius: MemeDockGeometry.group)
                    .strokeBorder(
                        dropTargeted ? palette.text : palette.separator,
                        style: StrokeStyle(lineWidth: dropTargeted ? 2 : 1.25, dash: [7, 5])
                    )
            }
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: dropTargeted)

            if let failure = model.failure {
                Text(failure)
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.error)
                    .multilineTextAlignment(.center)
                    .textSelection(.enabled)
                    .frame(maxWidth: 480)
            }

            Text(L10n.text(.creatorSDRHint))
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(24)
    }

    private func stage(_ palette: MemeDockPalette) -> some View {
        VStack(spacing: 16) {
            ZStack {
                if model.showingResult && model.result != nil {
                    CreatorResultView(model: model, collectionId: collectionId)
                        .transition(.opacity)
                } else {
                    VStack(spacing: 14) {
                        sourcePreview(palette)
                            .frame(minHeight: 220)
                        CreatorTimeline(model: model)
                        Text(L10n.text(.creatorCropHint))
                            .font(MemeDockTypography.caption)
                            .foregroundStyle(palette.secondary)
                    }
                    .transition(.opacity)
                }
            }
            .animation(reduceMotion ? nil : MemeDockMotion.fade, value: model.showingResult)

            if let failure = model.failure {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle")
                        .foregroundStyle(palette.error)
                    Text(failure)
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(palette.error)
                        .textSelection(.enabled)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            }
        }
        .padding(MemeDockGeometry.pageInset)
        .overlay {
            if dropTargeted {
                RoundedRectangle(cornerRadius: MemeDockGeometry.group)
                    .strokeBorder(palette.text, style: StrokeStyle(lineWidth: 2, dash: [8, 6]))
                    .background(palette.canvas.opacity(0.65), in: RoundedRectangle(cornerRadius: MemeDockGeometry.group))
                    .overlay {
                        VStack(spacing: 8) {
                            Image(systemName: "arrow.down.doc")
                                .font(.system(size: 30, weight: .medium))
                            Text(L10n.text(.creatorDropHint))
                                .font(MemeDockTypography.control)
                        }
                        .foregroundStyle(palette.text)
                    }
                    .padding(12)
                    .allowsHitTesting(false)
            }
        }
    }

    private func sourcePreview(_ palette: MemeDockPalette) -> some View {
        GeometryReader { geometry in
            let videoWidth = CGFloat(max(1, model.info?.width ?? 1))
            let videoHeight = CGFloat(max(1, model.info?.height ?? 1))
            let aspect = videoWidth / videoHeight
            let fitWidth = min(geometry.size.width, geometry.size.height * aspect)
            let fitHeight = fitWidth / aspect

            ZStack {
                RoundedRectangle(cornerRadius: 12)
                    .fill(.black)
                ZStack {
                    Group {
                        if model.playback.available {
                            CreatorVideoPlayer(player: model.playback.player)
                        } else if let image = model.fallbackImage {
                            Image(nsImage: image)
                                .resizable()
                                .scaledToFit()
                        }
                    }
                    .frame(width: max(1, fitWidth), height: max(1, fitHeight))
                    .clipped()

                    CreatorCropOverlay(
                        crop: $model.crop,
                        aspectLock: $model.aspectLock,
                        sourceSize: CGSize(width: videoWidth, height: videoHeight),
                        onTap: {
                            if model.playback.available { model.playback.toggle() }
                        },
                        onReset: {
                            model.resetCrop()
                        }
                    )
                    .frame(width: max(1, fitWidth), height: max(1, fitHeight))
                    .disabled(model.busy)
                }
            }
            .overlay(alignment: .bottom) {
                if !model.playback.available {
                    Text(L10n.text(.creatorFramePreview))
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(.black.opacity(0.72), in: RoundedRectangle(cornerRadius: 8))
                        .padding(10)
                }
            }
        }
    }

    private func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        guard !model.busy, !model.saving, let provider = providers.first else { return false }
        guard provider.canLoadObject(ofClass: URL.self) else { return false }
        _ = provider.loadObject(ofClass: URL.self) { url, _ in
            guard let url, url.isFileURL else { return }
            Task { @MainActor in
                model.open(url)
            }
        }
        return true
    }
}
