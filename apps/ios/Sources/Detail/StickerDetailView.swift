import MemeDockBridge
import SwiftUI

struct StickerDetailView: View {
    /// The model is created by the presenting layer after this view first appears.
    private struct LoadKey: Equatable {
        let pixels: Int
        let model: ObjectIdentifier?
    }
    let session: LibrarySession
    let model: StickerDetailModel?
    var transition = DetailTransition()
    private enum Editing { case title, note }
    @Environment(\.displayScale) private var displayScale
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var editing: Editing?
    @State private var draft = ""
    @State private var organizing = false
    @State private var restoring = false
    @State private var temporaryPreset: OutputPreset?
    @State private var showTitle = false
    @State private var purging = false
    @State private var choosingFormat = false
    @Environment(\.locale) private var locale

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
                            StickerPreviewView(model: model, transition: transition) {
                                Task { await model.load(pixels: pixels) }
                            }
                            .frame(height: geometry.size.height < 480 ? 200 : 360)
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
            .scrollDisabled(transition.dragging)
            .onScrollGeometryChange(for: Bool.self) {
                $0.contentOffset.y > 300
            } action: { _, value in
                showTitle = value
            }
            .task(id: LoadKey(pixels: pixels, model: model.map(ObjectIdentifier.init))) {
                updatePresentation()
                await model?.load(pixels: pixels)
            }
        }
        .safeAreaInset(edge: .top, spacing: 0) { topBar }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if let model, let detail = model.detail, model.phase == .ready {
                outputBar(model, detail: detail)
                    .offset(y: transition.chromeVisible ? 0 : 32)
                    .opacity(transition.chromeVisible ? 1 : 0)
            }
        }
        .sheet(isPresented: $organizing) {
            if let model, let catalog = session.organize, let detail = model.detail {
                StickerRelationsView(model: model, catalog: catalog, detail: detail)
            }
        }
        .sheet(isPresented: $choosingFormat) {
            ExportPresetPicker(selection: $temporaryPreset, animated: model?.detail?.asset.animated == true)
        }
        .confirmationDialog("Permanently Delete?", isPresented: $purging, titleVisibility: .visible) {
            Button("Permanently Delete", role: .destructive) {
                Task { if await model?.purge() == true { transition.dismiss() } }
            }
        } message: {
            Text("The original image will be permanently deleted. This cannot be undone.")
        }
        .alert(
            "Couldn’t Save Change",
            isPresented: Binding(
                get: { editing == nil && !organizing && !restoring && model?.managementError != nil },
                set: { if !$0 { model?.clearManagementError() } }
            )
        ) {
            Button("OK") { model?.clearManagementError() }
        } message: {
            Text(model?.managementError ?? "")
        }
        .sheet(isPresented: $restoring) {
            if let model {
                NavigationStack {
                    List {
                        Text("Restoring does not restore the previous collection and tag associations.")
                            .foregroundStyle(.secondary)
                        if let suggestions = model.restoreSuggestions {
                            Section("Previous Organization") {
                                if let collection = suggestions.collection {
                                    Label(collection.name, systemImage: "folder")
                                }
                                ForEach(suggestions.tags, id: \.id) { Label($0.name, systemImage: "tag") }
                            }
                        }
                        if let error = model.managementError { Text(error).foregroundStyle(.red) }
                        Button("Restore") { Task { if await model.restore() { restoring = false } } }
                            .disabled(model.isManaging)
                    }
                    .navigationTitle("Restore Sticker").navigationBarTitleDisplayMode(.inline)
                    .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { restoring = false } } }
                    .task { await model.loadRestoreSuggestions() }
                    .interactiveDismissDisabled(model.isManaging)
                }.presentationDetents([.medium, .large])
            }
        }
        .onChange(of: scenePhase) { updatePresentation() }
        .onChange(of: reduceMotion) { updatePresentation() }
        .onChange(of: transition.settled) { updatePresentation() }
        .onDisappear { model?.suspend() }
    }

    /// Playback waits for the opening transition to land and pauses while the page is dragged.
    private func updatePresentation() {
        model?.setPresentation(active: transition.settled && scenePhase == .active, reduceMotion: reduceMotion)
    }

    private var topBar: some View {
        HStack(spacing: 4) {
            Button(action: transition.dismiss) {
                Image(systemName: "chevron.backward").font(.system(size: 20, weight: .semibold))
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel(Text("Back"))
            Spacer(minLength: 0)
            if let model, let detail = model.detail { topActions(model, detail: detail) }
        }
        .overlay {
            Text(model?.detail?.sticker.title ?? "").font(.headline).lineLimit(1)
                .padding(.horizontal, 104)
                .opacity(showTitle ? 1 : 0)
                .accessibilityHidden(!showTitle)
        }
        .padding(.horizontal, 8)
        .frame(height: 44)
        .background {
            Rectangle().fill(.bar)
                .overlay(alignment: .bottom) { Divider() }
                .ignoresSafeArea(edges: .top)
                .opacity(showTitle ? 1 : 0)
        }
        .animation(reduceMotion ? nil : .easeInOut(duration: 0.2), value: showTitle)
        .offset(y: transition.chromeVisible ? 0 : -32)
        .opacity(transition.chromeVisible ? 1 : 0)
    }

    @ViewBuilder
    private func topActions(_ model: StickerDetailModel, detail: MemeDockBridge.StickerDetail) -> some View {
        if detail.sticker.lifecycle.deletedAt == nil {
            Button {
                Task { _ = await model.update(starred: !detail.sticker.starred) }
            } label: {
                Image(systemName: detail.sticker.starred ? "star.fill" : "star").font(.system(size: 19))
                    .foregroundStyle(detail.sticker.starred ? Color.orange : Color.accentColor)
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel(Text(detail.sticker.starred ? "Remove Favorite" : "Favorite"))
            .disabled(model.isManaging)
            Menu {
                Button("Organize", systemImage: "folder") { organizing = true }
                Button("Move to Trash", systemImage: "trash", role: .destructive) {
                    Task {
                        if await model.delete() {
                            session.feedback.post(String(localized: "Moved to Trash"), action: .viewTrash)
                            transition.dismiss()
                        }
                    }
                }
            } label: {
                Image(systemName: "ellipsis.circle").font(.system(size: 19)).frame(width: 44, height: 44)
            }
            .accessibilityLabel(Text("More Actions"))
            .disabled(model.isManaging || session.outputs?.isBusy == true)
        } else {
            Button(role: .destructive) {
                purging = true
            } label: {
                Image(systemName: "trash").font(.system(size: 19)).frame(width: 44, height: 44)
            }
            .tint(.red)
            .accessibilityLabel(Text("Permanently Delete"))
            .disabled(model.isManaging)
        }
    }

    private func detailContent(_ detail: MemeDockBridge.StickerDetail, model: StickerDetailModel) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(alignment: .firstTextBaseline) {
                if editing == .title {
                    editField(model, title: "Name")
                } else {
                    Button {
                        draft = detail.sticker.title
                        editing = .title
                    } label: {
                        Text(detail.sticker.title).font(.title2).fontWeight(.semibold).foregroundStyle(.primary)
                    }
                    .buttonStyle(.plain).disabled(detail.sticker.lifecycle.deletedAt != nil)
                }
                Spacer()
                if model.canControlPlayback {
                    Button {
                        Task { await model.playback.toggle() }
                    } label: {
                        Label(
                            playbackTitle(model.playback.state),
                            systemImage: model.playback.state == .playing ? "pause" : "play")
                    }
                    .disabled(model.playback.state == .loading || model.originalError != nil)
                }
            }
            if editing == .note {
                editField(model, title: "Note")
            } else if detail.sticker.lifecycle.deletedAt == nil {
                Button {
                    draft = detail.sticker.note
                    editing = .note
                } label: {
                    if detail.sticker.note.isEmpty {
                        Text("Add Note")
                    } else {
                        Text(detail.sticker.note).foregroundStyle(.secondary)
                    }
                }.buttonStyle(.plain)
            } else if !detail.sticker.note.isEmpty {
                Text(detail.sticker.note).foregroundStyle(.secondary)
            }
            if detail.sticker.lifecycle.deletedAt == nil {
                TagFlowLayout(spacing: 8) {
                    if let collection = detail.collection {
                        Button {
                            organizing = true
                        } label: {
                            Label(collection.name, systemImage: "folder")
                        }.buttonStyle(.bordered)
                    }
                    ForEach(detail.tags, id: \.id) { tag in
                        Button {
                            organizing = true
                        } label: {
                            Label(tag.name, systemImage: "tag")
                        }.buttonStyle(.bordered)
                    }
                    Button("Organize", systemImage: "plus") { organizing = true }.buttonStyle(.bordered)
                }
            }
            VStack(alignment: .leading, spacing: 12) {
                Text("File Information").font(.caption).foregroundStyle(.secondary)
                VStack(spacing: 12) {
                    LabeledContent("Original Filename", value: detail.sticker.originalName)
                    LabeledContent(
                        "Format",
                        value: detail.asset.mime.split(separator: "/").last.map { String($0).uppercased() }
                            ?? detail.asset.mime)
                    LabeledContent("Dimensions", value: "\(detail.asset.width) × \(detail.asset.height)")
                    LabeledContent("Size", value: ByteCountFormatStyle(locale: locale).format(detail.asset.byteSize))
                }
                .font(.subheadline)
            }
        }
    }

    private func editField(_ model: StickerDetailModel, title: LocalizedStringKey) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField(title, text: $draft, axis: .vertical).textFieldStyle(.roundedBorder)
            if let error = model.managementError { Text(error).font(.caption).foregroundStyle(.red) }
            HStack {
                Button("Cancel") {
                    editing = nil
                    model.clearManagementError()
                }
                Spacer()
                Button("Save") {
                    let field = editing
                    Task {
                        if await model.update(title: field == .title ? draft : nil, note: field == .note ? draft : nil)
                        {
                            editing = nil
                        }
                    }
                }.disabled(
                    model.isManaging
                        || (editing == .title && draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty))
            }
        }.disabled(model.isManaging)
    }

    @ViewBuilder
    private func outputBar(_ model: StickerDetailModel, detail: MemeDockBridge.StickerDetail) -> some View {
        VStack(spacing: 8) {
            if detail.sticker.lifecycle.deletedAt != nil {
                Button("Restore") { restoring = true }.buttonStyle(.borderedProminent).controlSize(.large)
                    .disabled(model.isManaging)
            } else if let output = session.outputs {
                let animated = detail.asset.animated
                let choice = temporaryPreset ?? session.outputPreferences.choice(.share, animated: animated)
                let ready = !output.isBusy && !model.isManaging && model.originalError == nil
                HStack {
                    Button {
                        choosingFormat = true
                    } label: {
                        Label(choice.title, systemImage: "chevron.up.chevron.down").font(.subheadline)
                            .lineLimit(1)
                    }
                    .disabled(!ready)
                    Spacer()
                    if output.isBusy {
                        Button("Cancel", systemImage: "xmark") { output.cancel() }
                            .labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                    } else {
                        Button("Copy", systemImage: "doc.on.doc") { request(.copy, detail: detail) }.labelStyle(
                            .iconOnly
                        ).frame(minWidth: 44, minHeight: 44).disabled(!ready)
                        Menu {
                            Button("Save to Photos") { request(.photos, detail: detail) }
                            Button("Save to Files") { request(.files, detail: detail) }
                        } label: {
                            Label("Save", systemImage: "square.and.arrow.down")
                        }.labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44).disabled(!ready)
                    }
                }.frame(minHeight: 44)
                Button {
                    request(.share, detail: detail)
                } label: {
                    ZStack {
                        Text(choice == .original ? "Share Original" : "Share")
                            .opacity(output.isBusy ? 0 : 1).accessibilityHidden(output.isBusy)
                        HStack {
                            Color.clear.frame(width: 20, height: 20).overlay {
                                if output.isBusy { ProgressView() }
                            }
                            ZStack {
                                Text("Saving…").opacity(output.saving ? 1 : 0).accessibilityHidden(!output.saving)
                                Text("Preparing…").opacity(output.saving ? 0 : 1).accessibilityHidden(output.saving)
                            }
                        }.opacity(output.isBusy ? 1 : 0).accessibilityHidden(!output.isBusy)
                    }.frame(maxWidth: .infinity, minHeight: 24)
                }
                .buttonStyle(.borderedProminent).controlSize(.large).disabled(!ready)
                if animated {
                    Button("Share First Frame (PNG)") { request(.share, detail: detail, firstFrame: true) }
                        .font(.subheadline).disabled(!ready)
                }
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 12).frame(maxWidth: 840).frame(maxWidth: .infinity)
        .background(.bar)
    }

    private func request(_ action: OutputAction, detail: MemeDockBridge.StickerDetail, firstFrame: Bool = false) {
        let preset = temporaryPreset ?? session.outputPreferences.choice(action, animated: detail.asset.animated)
        session.outputs?.start(action, sticker: detail.sticker.id, preset: preset, firstFrame: firstFrame)
    }

    private func playbackTitle(_ state: AnimatedPlayback.State) -> LocalizedStringKey {
        switch state {
        case .playing: "Pause"
        case .finished: "Replay"
        default: "Play"
        }
    }
}