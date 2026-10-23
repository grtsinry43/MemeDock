import SwiftUI
import MemeDockBridge

struct StickerDetailView: View {
    let session: LibrarySession
    let route: StickerDetailRoute
    var isVisible = true
    @Environment(\.displayScale) private var displayScale
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var model: StickerDetailModel?

    var body: some View {
        GeometryReader { geometry in
            let pixels = max(1, min(1280, Int(min(geometry.size.width, 680) * displayScale)))
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if let model {
                        switch model.phase {
                        case .unavailable(let message), .failed(let message):
                            ContentUnavailableView {
                                Label("Sticker Unavailable", systemImage: "exclamationmark.triangle")
                            } description: {
                                Text(message)
                            } actions: {
                                if case .failed = model.phase {
                                    Button("Retry") { Task { await model.load(pixels: pixels) } }
                                }
                            }
                        case .closed:
                            ContentUnavailableView("Library Closed", systemImage: "externaldrive")
                        case .idle, .loading, .ready:
                            StickerPreviewView(model: model) { Task { await model.load(pixels: pixels) } }
                                .frame(height: min(520, max(240, geometry.size.height * 0.55)))
                            if let detail = model.detail {
                                detailContent(detail, model: model)
                            } else {
                                ProgressView().frame(maxWidth: .infinity)
                            }
                        }
                    } else if session.phase != .ready {
                        ContentUnavailableView("Library Unavailable", systemImage: "externaldrive")
                    } else {
                        ProgressView().frame(maxWidth: .infinity, minHeight: 240)
                    }
                }
                .frame(maxWidth: 680, alignment: .leading)
                .padding(16)
                .frame(maxWidth: .infinity)
            }
            .task(id: pixels) {
                if model == nil { model = session.makeDetail(for: route) }
                model?.setPresentation(active: isVisible && scenePhase == .active, reduceMotion: reduceMotion)
                await model?.load(pixels: pixels)
            }
        }
        .navigationTitle("Sticker")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: scenePhase) { _, phase in
            model?.setPresentation(active: isVisible && phase == .active, reduceMotion: reduceMotion)
        }
        .onChange(of: reduceMotion) { _, reduced in
            model?.setPresentation(active: isVisible && scenePhase == .active, reduceMotion: reduced)
        }
        .onChange(of: isVisible) { _, visible in
            model?.setPresentation(active: visible && scenePhase == .active, reduceMotion: reduceMotion)
        }
        .onDisappear { model?.suspend() }
    }

    private func detailContent(_ detail: MemeDockBridge.StickerDetail, model: StickerDetailModel) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(alignment: .firstTextBaseline) {
                Text(detail.sticker.title).font(.title2).fontWeight(.semibold).textSelection(.enabled)
                Spacer()
                if model.canControlPlayback {
                    Button {
                        Task { await model.playback.toggle() }
                    } label: {
                        Label(playbackTitle(model.playback.state), systemImage: model.playback.state == .playing ? "pause" : "play")
                    }
                    .disabled(model.playback.state == .loading || model.originalError != nil)
                }
            }
            if !detail.sticker.note.isEmpty {
                Text(detail.sticker.note).foregroundStyle(.secondary).textSelection(.enabled)
            }
            if let collection = detail.collection {
                LabeledContent("Collection", value: collection.name)
            }
            if !detail.tags.isEmpty {
                LabeledContent("Tags", value: detail.tags.map(\.name).joined(separator: ", "))
            }
            DisclosureGroup("File Information") {
                VStack(spacing: 12) {
                    LabeledContent("Name", value: detail.sticker.originalName)
                    LabeledContent("Format", value: detail.asset.mime)
                    LabeledContent("Dimensions", value: "\(detail.asset.width) × \(detail.asset.height)")
                    LabeledContent("Size", value: ByteCountFormatter.string(fromByteCount: detail.asset.byteSize, countStyle: .file))
                }
                .font(.subheadline)
                .padding(.top, 12)
            }
        }
    }

    private func playbackTitle(_ state: AnimatedPlayback.State) -> LocalizedStringKey {
        switch state {
        case .playing: "Pause"
        case .finished: "Replay"
        default: "Play"
        }
    }
}
