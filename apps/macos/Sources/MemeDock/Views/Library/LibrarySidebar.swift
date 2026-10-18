import MemeDockBridge
import SwiftUI

/// Sections of the library. Collections and tags come from their catalogs, so a
/// rename, a deletion, or a restore shows up without reopening the window.
struct LibrarySidebar: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var model: LibraryModel
    @ObservedObject private var browser: StickerBrowser
    @ObservedObject private var collections: CollectionCatalog
    @ObservedObject private var tags: TagCatalog

    /// Which name the alert edits. Creating and renaming differ only in what
    /// they do with the typed name, so one alert serves both.
    private enum Naming: Identifiable {
        case newCollection
        case newTag
        case collection(CollectionMetadata)
        case tag(TagMetadata)

        var id: String {
            switch self {
            case .newCollection: return "new-collection"
            case .newTag: return "new-tag"
            case let .collection(value): return "collection-\(value.id)"
            case let .tag(value): return "tag-\(value.id)"
            }
        }

        var title: L10nKey {
            switch self {
            case .newCollection: return .newCollection
            case .newTag: return .newTag
            case .collection, .tag: return .rename
            }
        }

        var prompt: L10nKey {
            switch self {
            case .newCollection, .collection: return .collections
            case .newTag, .tag: return .tags
            }
        }

        var current: String {
            switch self {
            case .newCollection, .newTag: return ""
            case let .collection(value): return value.name
            case let .tag(value): return value.name
            }
        }
    }

    @State private var naming: Naming?
    @State private var draft = ""

    init(model: LibraryModel) {
        self.model = model
        _browser = ObservedObject(wrappedValue: model.browser)
        _collections = ObservedObject(wrappedValue: model.collections)
        _tags = ObservedObject(wrappedValue: model.tags)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 4) {
                Text("MemeDock")
                    .font(MemeDockTypography.sectionTitle)
                    .padding(.horizontal, 10)
                    .padding(.bottom, 16)
                Group {
                    sectionRow(.all, .all, "square.grid.2x2")
                    sectionRow(.starred, .starred, "star")
                    sectionRow(.recentlyUsed, .recentlyUsed, "clock")
                }
                collectionsSection
                tagsSection
                sectionRow(.deleted, .trash, "trash")
                    .contextMenu {
                        Button(L10n.text(.emptyTrash), role: .destructive) { Task { await model.requestEmptyTrash() } }
                            .disabled(model.purging)
                        Button(L10n.text(.retryPurgeCleanup)) { Task { await model.retryPurgeCleanup() } }
                    }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 16)
        }
        .font(MemeDockTypography.body)
        .background(MemeDockGlass(sidebar: true))
        .alert(L10n.text(naming?.title ?? .rename), isPresented: namingPresented) {
            TextField(L10n.text(naming?.prompt ?? .collections), text: $draft)
            Button(L10n.text(.cancel), role: .cancel) { naming = nil }
            Button(L10n.text(.confirm)) { commitName() }
        }
    }

    private var namingPresented: Binding<Bool> {
        Binding(
            get: { naming != nil },
            set: { if !$0 { naming = nil } }
        )
    }

    private var collectionsSection: some View {
        sidebarSection(.collections) {
            ForEach(collections.active, id: \.id) { collection in
                navigationRow(collection.name, filter: .collection(collection.id), symbol: "square.stack",
                              count: Int(collections.summary(collection.id)?.count ?? 0))
                    .contextMenu {
                        Button(L10n.text(.rename)) { begin(.collection(collection)) }
                        Button(L10n.text(.moveUp)) { move(collection, up: true) }
                            .disabled(collection.id == collections.active.first?.id)
                        Button(L10n.text(.moveDown)) { move(collection, up: false) }
                            .disabled(collection.id == collections.active.last?.id)
                        Divider()
                        Button(L10n.text(.delete), role: .destructive) { remove(collection) }
                    }
            }
            ForEach(collections.trashed, id: \.id) { collection in
                trashedRow(collection.name) { restore(collection) }
                    .contextMenu {
                        Button(L10n.text(.purge), role: .destructive) {
                            model.requestPurge([TrashTarget(kind: .collection, target: EntityReference(id: collection.id, generation: collection.lifecycle.generation), deletedRevision: collection.lifecycle.revision)])
                        }.disabled(model.purging)
                    }
            }
            createRow(.newCollection, .newCollection)
        }
    }

    private var tagsSection: some View {
        sidebarSection(.tags) {
            ForEach(tags.active, id: \.id) { tag in
                navigationRow(tag.name, filter: .tag(tag.id), symbol: "tag")
                    .contextMenu {
                        Button(L10n.text(.rename)) { begin(.tag(tag)) }
                        Divider()
                        Button(L10n.text(.delete), role: .destructive) { remove(tag) }
                    }
            }
            ForEach(tags.trashed, id: \.id) { tag in
                trashedRow(tag.name) { restore(tag) }
                    .contextMenu {
                        Button(L10n.text(.purge), role: .destructive) {
                            model.requestPurge([TrashTarget(kind: .tag, target: EntityReference(id: tag.id, generation: tag.lifecycle.generation), deletedRevision: tag.lifecycle.revision)])
                        }.disabled(model.purging)
                    }
            }
            createRow(.newTag, .newTag)
        }
    }

    private func sectionRow(_ filter: LibraryFilter, _ title: L10nKey, _ symbol: String) -> some View {
        navigationRow(L10n.text(title), filter: filter, symbol: symbol)
    }

    private func navigationRow(_ title: String, filter: LibraryFilter, symbol: String, count: Int? = nil) -> some View {
        let palette = MemeDockPalette(scheme)
        let selected = browser.filter == filter
        return Button { model.select(filter) } label: {
            HStack(spacing: 8) {
                Image(systemName: symbol)
                    .frame(width: 18)
                Text(title)
                    .lineLimit(1)
                Spacer(minLength: 4)
                if let count {
                    Text(String(count))
                        .font(MemeDockTypography.caption)
                        .monospacedDigit()
                        .contentTransition(reduceMotion ? .opacity : .numericText(value: Double(count)))
                        .animation(reduceMotion ? nil : .snappy(duration: 0.25), value: count)
                }
            }
            .foregroundStyle(selected ? palette.raised : palette.text)
            .padding(.horizontal, 10)
            .frame(minHeight: 36)
            .background(selected ? palette.text : .clear,
                        in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(title)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private func sidebarSection<Content: View>(_ title: L10nKey, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L10n.text(title))
                .font(MemeDockTypography.sectionLabel)
                .foregroundStyle(MemeDockPalette(scheme).secondary)
                .padding(.horizontal, 10)
                .padding(.top, 16)
                .padding(.bottom, 4)
            content()
        }
    }

    /// A deleted collection or tag keeps its name so a restore can be recognised;
    /// the button restores it because the row itself has no section to select.
    private func trashedRow(_ name: String, restore: @escaping () -> Void) -> some View {
        HStack(spacing: 6) {
            Label(name, systemImage: "trash")
                .foregroundStyle(.secondary)
            Spacer(minLength: 4)
            Button(L10n.text(.restore), action: restore)
                .buttonStyle(.borderless)
                .controlSize(.small)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
    }

    private func createRow(_ target: Naming, _ title: L10nKey) -> some View {
        Button { begin(target) } label: {
            Label(L10n.text(title), systemImage: "plus")
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(MemeDockButtonStyle(kind: .quiet))
        .foregroundStyle(MemeDockPalette(scheme).secondary)
    }

    private func begin(_ target: Naming) {
        draft = target.current
        naming = target
    }

    private func commitName() {
        let name = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let target = naming, !name.isEmpty else {
            naming = nil
            return
        }
        naming = nil
        switch target {
        case .newCollection:
            run { try await collections.create(name: name, before: nil) }
        case .newTag:
            run { try await tags.create(name: name) }
        case let .collection(collection):
            guard name != collection.name else { return }
            run { try await collections.rename(collection, name: name) }
        case let .tag(tag):
            guard name != tag.name else { return }
            run { try await tags.rename(tag, name: name) }
        }
    }

    private func move(_ collection: CollectionMetadata, up: Bool) {
        let items = collections.active
        guard let index = items.firstIndex(where: { $0.id == collection.id }) else { return }
        // `before` names the collection the moved one has to end up in front of,
        // so moving down means the one after the next.
        let before: String?
        if up {
            guard index > 0 else { return }
            before = items[index - 1].id
        } else {
            guard index + 1 < items.count else { return }
            before = index + 2 < items.count ? items[index + 2].id : nil
        }
        run { try await collections.move(collection, before: before) }
    }

    private func remove(_ collection: CollectionMetadata) {
        run { try await collections.delete(collection) }
    }

    private func remove(_ tag: TagMetadata) {
        run { try await tags.delete(tag) }
    }

    private func restore(_ collection: CollectionMetadata) {
        run { try await collections.restore(collection) }
    }

    private func restore(_ tag: TagMetadata) {
        run { try await tags.restore(tag) }
    }

    /// Everything the sidebar does is reported through the status banner, so a
    /// rejected name or a failed move never disappears silently.
    private func run(_ body: @escaping () async throws -> Void) {
        Task { await perform(model.status, body) }
    }
}
