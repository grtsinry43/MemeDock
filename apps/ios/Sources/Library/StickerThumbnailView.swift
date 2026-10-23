import CoreGraphics
import SwiftUI

struct StickerThumbnailView: View {
    private struct RequestKey: Hashable {
        let thumbnail: ThumbnailKey
        let retry: Int
    }
    let item: LibraryItem
    let store: ThumbnailStore
    @Environment(\.displayScale) private var displayScale
    @State private var image: CGImage?
    @State private var error: String?
    @State private var retry = 0

    private var key: ThumbnailKey { ThumbnailKey(item, pixels: Int(128 * displayScale)) }

    var body: some View {
        ZStack {
            if image == nil { Color(uiColor: .secondarySystemBackground) }
            if let image {
                Image(decorative: image, scale: displayScale)
                    .resizable()
                    .scaledToFit()
                    .padding(6)
            } else if error != nil {
                Button {
                    retry += 1
                } label: {
                    Label("Retry Thumbnail", systemImage: "arrow.clockwise")
                        .labelStyle(.iconOnly)
                }
                .accessibilityHint(error ?? "")
            } else {
                ProgressView()
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(.rect(cornerRadius: 8))
        .accessibilityLabel(item.sticker.title)
        .accessibilityValue(item.resource?.asset.animated == true ? String(localized: "Animated image") : "")
        .task(id: RequestKey(thumbnail: key, retry: retry)) { await load(force: retry > 0) }
        .onDisappear { image = nil }
    }

    private func load(force: Bool = false) async {
        image = nil
        error = nil
        do {
            let loaded = try await store.image(for: item, pixels: key.pixels, retry: force)
            try Task.checkCancellation()
            image = loaded
        } catch is CancellationError {
            // View disappearance and a newer thumbnail revision own the replacement request.
        } catch {
            guard !Task.isCancelled else { return }
            self.error = BridgeErrors.browseDescription(error)
        }
    }
}
