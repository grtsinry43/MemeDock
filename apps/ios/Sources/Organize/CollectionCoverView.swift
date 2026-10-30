import MemeDockBridge
import SwiftUI

struct CollectionCoverView: View {
    let summary: CollectionSummary
    let model: OrganizeModel
    let thumbnails: ThumbnailStore
    @Environment(\.displayScale) private var scale
    @State private var image: CGImage?
    @State private var failed = false

    var body: some View {
        ZStack {
            Rectangle().fill(.quaternary.opacity(0.35))
            if let image {
                Image(decorative: image, scale: scale).resizable().scaledToFit().padding(16)
            } else {
                Image(systemName: failed ? "photo.badge.exclamationmark" : "folder")
                    .font(.system(size: 36, weight: .light)).foregroundStyle(.secondary)
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .accessibilityHidden(true)
        .task(id: "\(summary.cover ?? "")-\(summary.thumbnailPath ?? "")") {
            image = nil
            failed = false
            do {
                if let item = try await model.coverItem(for: summary) {
                    let value = try await thumbnails.image(for: item, pixels: min(512, Int(240 * scale)))
                    try Task.checkCancellation()
                    image = value
                }
            } catch is CancellationError {
            } catch { failed = true }
        }
        .onDisappear { image = nil }
    }
}

struct CollectionCardContent: View {
    let summary: CollectionSummary
    let model: OrganizeModel
    let thumbnails: ThumbnailStore
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            CollectionCoverView(summary: summary, model: model, thumbnails: thumbnails)
            VStack(alignment: .leading, spacing: 4) {
                Text(summary.collection.name).font(.subheadline.weight(.medium)).foregroundStyle(.primary).lineLimit(1)
                Text("\(summary.count) stickers").font(.caption).foregroundStyle(.secondary)
            }.frame(maxWidth: .infinity, minHeight: 64, alignment: .leading).padding(.horizontal, 12)
        }
        .background(Color(uiColor: .secondarySystemGroupedBackground), in: .rect(cornerRadius: 20))
        .clipShape(.rect(cornerRadius: 20))
    }
}
