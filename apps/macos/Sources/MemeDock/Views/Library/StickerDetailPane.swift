import AppKit
import MemeDockBridge
import SwiftUI

/// The right column: what the selected sticker is, what it belongs to, and what
/// can be done with it. Edits go through the core and come back as the stored
/// value, so a rejected name or a stale generation is visible instead of assumed.
struct StickerDetailPane: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var model: LibraryModel
    @ObservedObject private var detail: StickerDetailModel
    @ObservedObject private var collections: CollectionCatalog
    @ObservedObject private var tags: TagCatalog
    @ObservedObject private var thumbnails: ThumbnailStore
    @ObservedObject private var actions: StickerActions

    private enum Field {
        case title
        case note
    }

    @State private var title = ""
    @State private var note = ""
    @State private var fileInfoExpanded = false
    @State private var showingTagPicker = false
    @State private var newTagName = ""
    @State private var creatingTag = false
    @State private var tagCreateError: String?
    @FocusState private var editing: Field?

    init(model: LibraryModel) {
        self.model = model
        _detail = ObservedObject(wrappedValue: model.detail)
        _collections = ObservedObject(wrappedValue: model.collections)
        _tags = ObservedObject(wrappedValue: model.tags)
        _thumbnails = ObservedObject(wrappedValue: model.thumbnails)
        _actions = ObservedObject(wrappedValue: model.actions)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            ScrollView {
                VStack(alignment: .leading, spacing: MemeDockGeometry.sectionGap) {
                    if detail.loading {
                        loadingState
                    } else if let sticker = detail.sticker {
                        preview(sticker)
                        titleField(sticker)
                        if sticker.originalPath == nil {
                            Label(L10n.text(.restoreRequired), systemImage: "exclamationmark.triangle")
                                .font(MemeDockTypography.caption)
                                .foregroundStyle(.orange)
                        }
                        actionsSection
                        organizationSection(sticker)
                        if sticker.sticker.lifecycle.deletedAt != nil {
                            trashSection
                        }
                        fileSection(sticker)
                        if let failure = detail.failure {
                            errorState(failure)
                        }
                    } else if let failure = detail.failure {
                        errorState(failure)
                    } else {
                        loadingState
                    }
                }
                .padding(MemeDockGeometry.pageInset)
            }
        }
        .background(MemeDockPalette(scheme).raised)
        .task(id: detail.id) { syncFields() }
        .onChange(of: detail.id) { _, _ in
            fileInfoExpanded = false
            showingTagPicker = false
            newTagName = ""
            tagCreateError = nil
        }
        .onChange(of: detail.sticker) { syncFields() }
        .onChange(of: editing) { previous, _ in
            // Leaving a field by clicking elsewhere commits it, so a typed title
            // is not lost to a pane that was never submitted.
            guard !detail.loading, let sticker = detail.sticker, let previous else { return }
            switch previous {
            case .title: commitTitle(sticker)
            case .note: commitNote(sticker)
            }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text(L10n.text(.details))
                .font(MemeDockTypography.rowTitle)
            Spacer()
            if let sticker = detail.sticker {
                Button {
                    let starred = !sticker.sticker.starred
                    run { await detail.patch(starred: starred) }
                } label: {
                    Image(systemName: sticker.sticker.starred ? "star.fill" : "star")
                        .foregroundStyle(sticker.sticker.starred ? MemeDockPalette(scheme).star : MemeDockPalette(scheme).secondary)
                }
                .buttonStyle(.borderless)
                .disabled(detail.loading)
                .help(L10n.text(sticker.sticker.starred ? .unstar : .star))
                moreMenu(sticker)
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
        .background(MemeDockPalette(scheme).raised)
    }

    private func preview(_ sticker: StickerDetail) -> some View {
        OriginalPreview(
            path: sticker.originalPath,
            fallback: previewFallback(sticker),
            animated: sticker.asset.animated,
            mime: sticker.asset.mime
        )
        .frame(height: 190)
    }

    private func previewFallback(_ sticker: StickerDetail) -> NSImage? {
        guard let item = detail.item else { return nil }
        return thumbnails.image(for: item)
    }

    private func fileSection(_ sticker: StickerDetail) -> some View {
        DisclosureGroup(isExpanded: $fileInfoExpanded) {
            VStack(alignment: .leading, spacing: 8) {
                LabeledContent(L10n.text(.originalName)) {
                    Text(sticker.sticker.originalName)
                        .textSelection(.enabled)
                        .lineLimit(2)
                        .truncationMode(.middle)
                }
                LabeledContent(L10n.text(.size), value: Format.size(sticker.asset.byteSize))
                LabeledContent(L10n.text(.dimensions), value: "\(sticker.asset.width) × \(sticker.asset.height)")
                LabeledContent(L10n.text(.format), value: sticker.asset.animated ? L10n.text(.animated) : L10n.text(.staticImage))
                LabeledContent(L10n.text(.createdAt), value: Format.timestamp(sticker.sticker.lifecycle.createdAt))
            }
            .font(MemeDockTypography.caption)
            .padding(.top, 8)
        } label: {
            Text(L10n.text(.fileInfo))
                .font(MemeDockTypography.sectionLabel)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
        }
        .disclosureGroupStyle(MemeDockDisclosureStyle())
    }

    private func titleField(_ sticker: StickerDetail) -> some View {
        TextField(L10n.text(.title), text: $title)
            .textFieldStyle(MemeDockFieldStyle(focused: editing == .title))
            .focused($editing, equals: .title)
            .onSubmit { commitTitle(sticker) }
    }

    private var loadingState: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.text(.loading))
                .font(MemeDockTypography.caption)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }

    private func organizationSection(_ sticker: StickerDetail) -> some View {
        section(L10n.text(.organization)) {
            collectionPicker(sticker)
            tagPicker(sticker)
            TextField(L10n.text(.addNote), text: $note, axis: .vertical)
                .lineLimit(2 ... 6)
                .textFieldStyle(MemeDockFieldStyle(focused: editing == .note))
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
        VStack(alignment: .leading, spacing: 8) {
            Text(L10n.text(.tags))
                .font(MemeDockTypography.sectionLabel)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
            MemeDockTagLayout {
                ForEach(sticker.tags, id: \.id) { tag in
                    Button { toggle(sticker, tag) } label: {
                        tagLabel(tag.name, symbol: "xmark")
                    }
                    .buttonStyle(.plain)
                    .help(L10n.text(.removeTag, tag.name))
                }
                Button { showingTagPicker = true } label: {
                    tagLabel(L10n.text(.addTags), symbol: "plus")
                }
                .buttonStyle(.plain)
                .popover(isPresented: $showingTagPicker) { tagChoices(sticker) }
            }
            .disabled(detail.organizing || creatingTag || sticker.sticker.lifecycle.deletedAt != nil)
        }
    }

    private func tagLabel(_ name: String, symbol: String, selected: Bool = false) -> some View {
        HStack(spacing: 6) {
            Text(name).lineLimit(1).truncationMode(.middle)
            Image(systemName: symbol).font(.system(size: 10, weight: .medium))
        }
        .font(MemeDockTypography.caption)
        .foregroundStyle(selected ? MemeDockPalette(scheme).raised : MemeDockPalette(scheme).text)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(selected ? MemeDockPalette(scheme).text : MemeDockPalette(scheme).input,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
        .contentShape(Rectangle())
    }

    private func tagChoices(_ sticker: StickerDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L10n.text(.tags)).font(MemeDockTypography.rowTitle)
            ScrollView {
                MemeDockTagLayout {
                    ForEach(tags.active, id: \.id) { tag in
                        let selected = contains(sticker, tag)
                        Button { toggle(sticker, tag) } label: {
                            tagLabel(tag.name, symbol: selected ? "checkmark" : "plus", selected: selected)
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(selected ? .isSelected : [])
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxHeight: 180)
            .disabled(detail.organizing || creatingTag)
            HStack(spacing: 8) {
                TextField(L10n.text(.newTag), text: $newTagName)
                    .textFieldStyle(MemeDockFieldStyle())
                    .onSubmit { createTag(for: sticker.sticker.id) }
                Button { createTag(for: sticker.sticker.id) } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(MemeDockButtonStyle(kind: .quiet))
                .help(L10n.text(.newTag))
                .disabled(newTagName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .disabled(detail.organizing || creatingTag)
            if detail.organizing || creatingTag { ProgressView().controlSize(.small) }
            if let error = tagCreateError ?? tags.failure ?? detail.failure {
                Text(error).font(MemeDockTypography.caption).foregroundStyle(MemeDockPalette(scheme).error)
            }
        }
        .padding(16)
        .frame(width: 300)
        .font(MemeDockTypography.body)
        .presentationCornerRadius(MemeDockGeometry.dialog)
    }

    private func createTag(for stickerId: String) {
        let name = newTagName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, !creatingTag, !detail.organizing else { return }
        creatingTag = true
        tagCreateError = nil
        Task {
            defer { creatingTag = false }
            do {
                let tag = try await tags.create(name: name)
                await detail.addTag(tag, stickerId: stickerId)
                if detail.id == stickerId, detail.failure == nil { newTagName = "" }
            } catch { tagCreateError = describe(error) }
        }
    }

    private var trashSection: some View {
        section(L10n.text(.trash)) {
            Text(L10n.text(.restoreHint))
                .font(MemeDockTypography.body)
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
        VStack(alignment: .leading, spacing: 8) {
            if let item = detail.item {
                Menu {
                    Picker(L10n.text(.exportPreset), selection: $actions.outputPreset) {
                        ForEach([ExportPreset.original, .compatiblePng, .whiteBackground, .smallJpeg], id: \.self) { preset in
                            Text(StickerExport.label(preset)).tag(preset)
                        }
                    }
                    .pickerStyle(.inline)
                } label: {
                    HStack(spacing: 6) {
                        Text(StickerExport.label(actions.outputPreset)).font(MemeDockTypography.control)
                        Image(systemName: "chevron.down").font(.system(size: 10, weight: .medium))
                    }
                    .frame(height: MemeDockGeometry.controlHeight)
                }
                .menuStyle(.button)
                .buttonStyle(.plain)
                .menuIndicator(.hidden)
                .help(StickerExport.hint(actions.outputPreset))
                if item.animated, actions.outputPreset != .original {
                    Text(L10n.text(.animatedUsesOriginal))
                        .font(MemeDockTypography.caption)
                        .foregroundStyle(MemeDockPalette(scheme).secondary)
                }
                Button { run { try await model.actions.copyImage(item) } } label: {
                    HStack(spacing: 8) {
                        Label(L10n.text(.copyImage), systemImage: "doc.on.doc")
                        Spacer(minLength: 8)
                        if actions.busy {
                            ProgressView()
                                .controlSize(.small)
                                .tint(MemeDockPalette(scheme).raised)
                                .accessibilityLabel(L10n.text(.applying))
                        } else {
                            Text("⌘C")
                                .font(MemeDockTypography.caption)
                                .opacity(0.65)
                        }
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(MemeDockButtonStyle(kind: .primary))
                HStack(spacing: 12) {
                    Button { run { try await model.actions.copyFile(item) } } label: {
                        actionIcon(.copyFile, "doc")
                    }
                    .buttonStyle(.plain)
                    Button { run { try await model.actions.save(item) } } label: {
                        actionIcon(.saveAs, "arrow.down.to.line")
                    }
                    .buttonStyle(.plain)
                    Button { run { try await model.actions.share(item) } } label: {
                        actionIcon(.share, "square.and.arrow.up")
                    }
                    .buttonStyle(.plain)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(MemeDockPalette(scheme).secondary)
            }
        }
        .disabled(actions.busy || detail.loading || detail.sticker?.originalPath == nil)
    }

    private func actionIcon(_ title: L10nKey, _ symbol: String) -> some View {
        Label(L10n.text(title), systemImage: symbol)
            .labelStyle(.iconOnly)
            .frame(width: MemeDockGeometry.controlHeight, height: MemeDockGeometry.controlHeight)
            .contentShape(Rectangle())
            .help(L10n.text(title))
    }

    private func moreMenu(_ sticker: StickerDetail) -> some View {
        Menu {
            if sticker.asset.animated, let item = detail.item {
                Button(L10n.text(.firstFrame)) {
                    run { try await model.actions.copyImage(item, firstFrame: true) }
                }
                .disabled(sticker.originalPath == nil)
                Divider()
            }
            if sticker.sticker.lifecycle.deletedAt == nil {
                Button(L10n.text(.deleteStickerTitle), role: .destructive) {
                    run { await detail.delete() }
                }
            } else {
                Button(L10n.text(.restoreStickerTitle)) { run { await detail.restore() } }
            }
        } label: {
            actionIcon(.more, "ellipsis")
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .disabled(actions.busy || detail.loading)
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(MemeDockTypography.sectionLabel)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func errorState(_ message: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(.orange)
            Text(message)
                .font(MemeDockTypography.body)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }

    private func contains(_ sticker: StickerDetail, _ tag: TagMetadata) -> Bool {
        sticker.tags.contains { $0.id == tag.id }
    }

    /// The core replaces the whole tag set, so a single toggle is sent as the
    /// set the sticker should end up with.
    private func toggle(_ sticker: StickerDetail, _ tag: TagMetadata) {
        Task { await detail.toggleTag(tag, stickerId: sticker.sticker.id) }
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
    @Environment(\.colorScheme) private var scheme
    let path: String?
    let fallback: NSImage?
    let animated: Bool
    let mime: String

    @StateObject private var playback = AnimatedPlayback()
    @State private var still: NSImage?

    private var plays: Bool { AnimatedPlayback.plays(animated: animated, mime: mime) }

    var body: some View {
        VStack(spacing: 8) {
            picture
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            if plays || note != nil {
                HStack(spacing: 8) {
                    if let note {
                        Text(note)
                            .font(MemeDockTypography.caption)
                            .foregroundStyle(MemeDockPalette(scheme).secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    Spacer(minLength: 0)
                    if plays { control }
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 8)
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
            Label(
                L10n.text(playback.state == .playing ? .pauseAnimation : .playAnimation),
                systemImage: playback.state == .playing ? "pause.fill" : "play.fill"
            )
                .font(MemeDockTypography.caption)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
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
