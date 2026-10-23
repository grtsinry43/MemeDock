import SwiftUI

struct StickerPreviewView: View {
    let model: StickerDetailModel
    let retry: () -> Void
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        VStack(spacing: 12) {
            ZStack {
                if let frame = model.playback.frame {
                    Image(decorative: frame, scale: displayScale).resizable().scaledToFit()
                } else if let placeholder = model.placeholder {
                    Image(decorative: placeholder, scale: displayScale).resizable().scaledToFit()
                } else if model.phase == .loading || model.playback.state == .loading {
                    ProgressView()
                } else {
                    Image(systemName: "photo").font(.largeTitle).foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .accessibilityLabel(model.detail?.sticker.title ?? String(localized: "Sticker Preview"))

            if let error = error {
                VStack(spacing: 8) {
                    Text(error).font(.subheadline).foregroundStyle(.secondary)
                    if model.placeholder != nil {
                        Text("Showing a thumbnail only.").font(.caption).foregroundStyle(.secondary)
                    }
                    Button("Retry Preview", action: retry)
                }
            }
        }
    }

    private var error: String? {
        if let error = model.originalError { return error }
        if case .failed(let message) = model.playback.state { return message }
        return nil
    }
}
