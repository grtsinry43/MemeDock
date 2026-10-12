import AppKit
import MemeDockBridge
import SwiftUI

/// The right column: what the selected sticker is, what it belongs to, and what
/// can be done with it. Edits go through the core and come back as the stored
/// value, so a rejected name or a stale generation is visible instead of assumed.
struct StickerDetailPane: View {
    @ObservedObject var model: LibraryModel
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var collections: CollectionCatalog
    @ObservedObject private var tags: TagCatalog
    @ObservedObject private var thumbnails: ThumbnailStore

    private enum Field {
        case title
        case note
    }

    @State private var title = ""
    @State private var note = ""
    @FocusState private var editing: Field?

    init(model: LibraryModel) {
        self.model = model
        _detail = ObservedObject(wrappedValue: model.detail)
        _collections = ObservedObject(wrappedValue: model.collections)
        _tags = ObservedObject(wrappedValue: model.tags)
        _thumbnails = ObservedObject(wrappedValue: model.thumbnails)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let sticker = detail.sticker {
                        preview(sticker)
                        fileSection(sticker)
                        organizationSection(sticker)
                        if sticker.sticker.lifecycle.deletedAt != nil {
                            trashSection
                        }
                        actionsSection
                    } else if let failure = detail.failure {
                        errorState(failure)
                    } else {
                        ProgressView()
                            .controlSize(.small)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 40)
                    }
                }
                .padding(16)
            }
        }
        .task(id: detail.id) { syncFields() }
        .onChange(of: detail.sticker) { syncFields() }
        .onChange(of: editing) { previous, _ in
            // Leaving a field by clicking elsewhere commits it, so a typed title
            // is not lost to a pane that was never submitted.
            guard let sticker = detail.sticker, let previous else { return }
            switch previous {
            case .title: commitTitle(sticker)
            case .note: commitNote(sticker)
            }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text(L10n.text(.details))
                .font(.headline)
            Spacer()
            if let sticker = detail.sticker {
                Button {
                    let starred = !sticker.sticker.starred
                    run { await detail.patch(starred: starred) }
                } label: {
                    Image(systemName: sticker.sticker.starred ? "star.fill" : "star")
                        .foregroundStyle(sticker.sticker.starred ? Color.yellow : Color.secondary)
                }
                .buttonStyle(.borderless)
                .help(L10n.text(sticker.sticker.starred ? .unstar : .star))
            }
            Button {
                detail.clear()
            } label: {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
            .help(L10n.text(.close))
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    private func preview(_ sticker: StickerDetail) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: 8)
                .fill(.quaternary)
            OriginalPreview(
                path: sticker.originalPath,
                fallback: previewFallback(sticker),
                animated: sticker.asset.animated,
                mime: sticker.asset.mime
            )
        }
        .frame(height: 190)
    }

    private func previewFallback(_ sticker: StickerDetail) -> NSImage? {
        guard let item = detail.item else { return nil }
        return thumbnails.image(for: item)
    }

    private func fileSection(_ sticker: StickerDetail) -> some View {
        section(L10n.text(.fileInfo)) {
            TextField(L10n.text(.title), text: $title)
                .textFieldStyle(.roundedBorder)
                .focused($editing, equals: .title)
                .onSubmit { commitTitle(sticker) }
            LabeledContent(L10n.text(.originalName), value: sticker.sticker.originalName)
            LabeledContent(L10n.text(.size), value: Format.size(sticker.asset.byteSize))
            LabeledContent(L10n.text(.dimensions), value: "\(sticker.asset.width) × \(sticker.asset.height)")
            LabeledContent(L10n.text(.format), value: sticker.asset.animated ? L10n.text(.animated) : L10n.text(.staticImage))
            LabeledContent(L10n.text(.createdAt), value: Format.timestamp(sticker.sticker.lifecycle.createdAt))
            if sticker.originalPath == nil {
                Label(L10n.text(.restoreRequired), systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.orange)
            }
        }
    }

    private func organizationSection(_ sticker: StickerDetail) -> some View {
        section(L10n.text(.organization)) {
            collectionPicker(sticker)
            tagPicker(sticker)
            TextField(L10n.text(.addNote), text: $note, axis: .vertical)
                .lineLimit(2 ... 6)
                .textFieldStyle(.roundedBorder)
                .focused($editing, equals: .note)
                .onSubmit { commitNote(sticker) }
        }
    }

    private func collectionPicker(_ sticker: StickerDetail) -> some View {
        HStack(spacing: 8) {
            Text(L10n.text(.collections))
                .foregroundStyle(.secondary)
            Spacer()
            Menu(sticker.collection?.name ?? L10n.text(.noCollection)) {
                Button(L10n.text(.noCollection)) { assign(.cleared) }
                Divider()
                ForEach(collections.active, id: \.id) { collection in
                    Button(collection.name) {
                        assign(.assign(EntityReference(id: collection.id, generation: collection.lifecycle.generation)))
                    }
                }
            }
            .frame(maxWidth: 170, alignment: .trailing)
        }
    }

    private func tagPicker(_ sticker: StickerDetail) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Text(L10n.text(.tags))
                    .foregroundStyle(.secondary)
                Spacer()
                Menu(L10n.text(.addTags)) {
                    ForEach(modelTags, id: \.id) { tag in
                        Button(toggleSymbol(contains(sticker, tag)) + tag.name) {
                            toggle(sticker, tag)
                        }
                    }
                }
                .frame(maxWidth: 170, alignment: .trailing)
            }
            if !sticker.tags.isEmpty {
                Text(sticker.tags.map(\.name).joined(separator: "、"))
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var trashSection: some View {
        section(L10n.text(.trash)) {
            Text(L10n.text(.restoreHint))
                .font(.callout)
                .foregroundStyle(.secondary)
            Button(L10n.text(.restoreStickerTitle)) {
                run { await detail.restore() }
            }
            if detail.suggestions != nil {
                Button(L10n.text(.restoreOrganization)) {
                    run { await detail.applySuggestions() }
                }
            }
        }
    }

    private var actionsSection: some View {
        section(L10n.text(.copy)) {
            if let item = detail.item {
                HStack(spacing: 8) {
                    Button(L10n.text(.copyImage)) { run { try await model.actions.copyImage(item) } }
                    Button(L10n.text(.copyFile)) { run { try await model.actions.copyFile(item) } }
                }
                if item.animated {
                    Button(L10n.text(.firstFrame)) {
                        run { try await model.actions.copyImage(item, firstFrame: true) }
                    }
                }
                HStack(spacing: 8) {
                    Menu(L10n.text(.saveAs)) {
                        presetButtons { preset in
                            run { try await model.actions.save(item, preset: preset) }
                        }
                    }
                    Menu(L10n.text(.share)) {
                        presetButtons { preset in
                            run { try await model.actions.share(item, preset: preset) }
                        }
                    }
                }
                if detail.sticker?.sticker.lifecycle.deletedAt != nil {
                    Button(L10n.text(.restoreStickerTitle)) { run { await detail.restore() } }
                } else {
                    Button(L10n.text(.deleteStickerTitle), role: .destructive) {
                        run { await detail.delete() }
                    }
                }
            }
        }
    }

    private func presetButtons(_ action: @escaping (ExportPreset) -> Void) -> some View {
        ForEach([ExportPreset.original, .compatiblePng, .whiteBackground, .smallJpeg], id: \.self) { preset in
            Button(StickerExport.label(preset)) { action(preset) }
        }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.subheadline.weight(.semibold))
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func errorState(_ message: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(.orange)
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }

    private var modelTags: [TagMetadata] {
        tags.active
    }

    private func contains(_ sticker: StickerDetail, _ tag: TagMetadata) -> Bool {
        sticker.tags.contains { $0.id == tag.id }
    }

    private func toggleSymbol(_ on: Bool) -> String {
        on ? "✓ " : ""
    }

    /// The core replaces the whole tag set, so a single toggle is sent as the
    /// set the sticker should end up with.
    private func toggle(_ sticker: StickerDetail, _ tag: TagMetadata) {
        var tags = sticker.tags
        if contains(sticker, tag) {
            tags.removeAll { $0.id == tag.id }
        } else {
            tags.append(tag)
        }
        run { await detail.assign(tags: tags) }
    }

    private func assign(_ collection: LibrarySession.CollectionAssignment) {
        run { await detail.assign(collection: collection) }
    }

    private func commitTitle(_ sticker: StickerDetail) {
        guard title != sticker.sticker.title else { return }
        run { await detail.patch(title: title) }
    }

    private func commitNote(_ sticker: StickerDetail) {
        guard note != sticker.sticker.note else { return }
        run { await detail.patch(note: note) }
    }

    /// Text fields are filled from the stored value, and committing on focus loss
    /// keeps a typed title from being lost when the pane is left by clicking away.
    private func syncFields() {
        title = detail.sticker?.sticker.title ?? ""
        note = detail.sticker?.sticker.note ?? ""
    }

    private func run(_ body: @escaping () async throws -> Void) {
        Task { await perform(model.status, body) }
    }
}

/// The stored original at its real size. An animated GIF or WebP is played (see
/// `AnimatedPlayback`). Still-image file reads happen off the main thread;
/// AppKit image creation and view updates stay on the main actor.
private struct OriginalPreview: View {
    let path: String?
    let fallback: NSImage?
    let animated: Bool
    let mime: String

    @StateObject private var playback = AnimatedPlayback()
    @State private var still: NSImage?

    private var plays: Bool { AnimatedPlayback.plays(animated: animated, mime: mime) }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            picture
            if plays {
                control
                    .padding(8)
            }
        }
        .overlay(alignment: .bottom) {
            if let note {
                Text(note)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.thinMaterial, in: Capsule())
                    .padding(8)
            }
        }
        .task(id: path) { await load() }
        .onDisappear { playback.stop() }
    }

    @ViewBuilder
    private var picture: some View {
        // Until the first frame arrives the thumbnail stands in, which is what
        // the Linux client shows before its playback starts too.
        if let frame = playback.frame {
            rendered(frame)
        } else if let still {
            rendered(still)
        } else if let fallback {
            rendered(fallback)
        } else {
            Image(systemName: "photo")
                .foregroundStyle(.tertiary)
        }
    }

    private func rendered(_ image: NSImage) -> some View {
        Image(nsImage: image)
            .resizable()
            .interpolation(.high)
            .scaledToFit()
            .padding(6)
    }

    private var control: some View {
        Button {
            playback.toggle()
        } label: {
            Image(systemName: playback.state == .playing ? "pause.fill" : "play.fill")
                .font(.caption)
                .padding(6)
                .background(.thinMaterial, in: Circle())
        }
        .buttonStyle(.plain)
        .help(L10n.text(playback.state == .playing ? .pauseAnimation : .playAnimation))
    }

    /// What the pane cannot show: a PNG animation, which is only ever a still
    /// frame, and the two ways decoding a real animation can fail.
    private var note: String? {
        if AnimatedPlayback.isStillAnimation(animated: animated, mime: mime) {
            return L10n.text(.animatedFirstFrame)
        }
        switch playback.state {
        case .broken: return L10n.text(.thumbnailFailed)
        case .limit: return L10n.text(.errorResourceLimit)
        case .idle, .playing, .paused: return nil
        }
    }

    @MainActor
    private func load() async {
        guard !Task.isCancelled else { return }
        playback.load(path: path, animated: animated, mime: mime)
        still = nil
        // The player supplies an animation's frames, so only still images are
        // decoded here.
        guard let path, !plays else { return }
        let read = Task.detached(priority: .userInitiated) {
            try Task.checkCancellation()
            let data = try Data(contentsOf: URL(fileURLWithPath: path))
            try Task.checkCancellation()
            return data
        }
        let data = try? await withTaskCancellationHandler {
            try await read.value
        } onCancel: {
            read.cancel()
        }
        guard !Task.isCancelled else { return }
        still = data.flatMap { NSImage(data: $0) }
    }
}
