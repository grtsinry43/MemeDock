import CoreGraphics
import SwiftUI

struct StickerThumbnailView: View {
    private struct RequestKey: Hashable {
        let thumbnail: ThumbnailKey
        let retry: Int
    }
    let item: LibraryItem
    let store: ThumbnailStore
    let open: () -> Void
    @Environment(\.displayScale) private var displayScale
    @State private var image: CGImage?
    @State private var error: String?
    @State private var retry = 0

    private var key: ThumbnailKey { ThumbnailKey(item, pixels: Int(128 * displayScale)) }

    var body: some View {
        ZStack {
            Button(action: open) {
                ZStack {
                    if image == nil { Color(uiColor: .secondarySystemBackground) }
                    if let image {
                        Image(decorative: image, scale: displayScale)
                            .resizable().scaledToFit().padding(6)
                    } else if error == nil {
                        ProgressView()
                    } else {
                        Image(systemName: "photo").foregroundStyle(.secondary)
                    }
                }
                .aspectRatio(1, contentMode: .fit)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(item.sticker.title)
            .accessibilityValue(item.resource?.asset.animated == true ? String(localized: "Animated image") : "")
            if error != nil {
                VStack {
                    Spacer()
                    HStack {
                        Spacer()
                        Button { retry += 1 } label: {
                            Label("Retry Thumbnail", systemImage: "arrow.clockwise")
                                .labelStyle(.iconOnly).frame(width: 44, height: 44)
                        }
                        .accessibilityHint(error ?? "")
                    }
                }
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(.rect(cornerRadius: 8))
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
