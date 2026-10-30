import MemeDockBridge
import SwiftUI

struct OrganizeView: View {
    let model: OrganizeModel
    let thumbnails: ThumbnailStore
    @Environment(\.dynamicTypeSize) private var typeSize
    @State private var editing: GroupEditRequest?
    @State private var deleting: GroupRoute?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if let error = model.error {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(error).foregroundStyle(.secondary)
                        Button("Retry") { Task { await model.refresh() } }
                    }
                }
                VStack(alignment: .leading, spacing: 16) {
                    HStack {
                        Text("Collections").font(.title2.bold())
                        Spacer()
                        NavigationLink("See All", value: OrganizeDestination.collections)
                    }
                    if model.isLoading && model.collections.isEmpty {
                        ProgressView().frame(maxWidth: .infinity, minHeight: 160)
                    } else if model.collections.isEmpty {
                        ContentUnavailableView {
                            Label("Keep Reactions Together", systemImage: "folder.badge.plus")
                        } description: {
                            Text("Create a collection for the stickers you reach for together.")
                        } actions: {
                            Button("New Collection") { editing = .newCollection }
                        }
                    } else {
                        ScrollView(.horizontal) {
                            LazyHStack(alignment: .top, spacing: 12) {
                                ForEach(model.collections, id: \.collection.id) { summary in
                                    NavigationLink(
                                        value: OrganizeDestination.group(collectionRoute(summary.collection))
                                    ) {
                                        CollectionCardContent(summary: summary, model: model, thumbnails: thumbnails)
                                    }
                                    .buttonStyle(.plain)
                                    .contextMenu {
                                        Button("Rename", systemImage: "pencil") {
                                            editing = .rename(collectionRoute(summary.collection))
                                        }
                                        NavigationLink("Reorder Collections", value: OrganizeDestination.reorderCollections)
                                        Button("Move to Trash", systemImage: "trash", role: .destructive) {
                                            deleting = collectionRoute(summary.collection)
                                        }
                                    }
                                    .frame(width: typeSize.isAccessibilitySize ? 210 : 156)
                                }
                                Button {
                                    editing = .newCollection
                                } label: {
                                    VStack(alignment: .leading, spacing: 0) {
                                        Rectangle().fill(.clear).aspectRatio(1, contentMode: .fit)
                                            .overlay {
                                                Image(systemName: "plus").font(.system(size: 36, weight: .light))
                                            }
                                        Text("New Collection").font(.subheadline.weight(.medium))
                                            .frame(maxWidth: .infinity, minHeight: 64, alignment: .leading).padding(
                                                .horizontal, 12)
                                    }.foregroundStyle(Color.accentColor)
                                        .background(Color.accentColor.opacity(0.12), in: .rect(cornerRadius: 20))
                                }
                                .buttonStyle(.plain).frame(width: typeSize.isAccessibilitySize ? 210 : 156)
                            }
                        }.scrollIndicators(.hidden)

                    }
                }
                if let editing, isCollectionEdit(editing) {
                    GroupInlineEditor(model: model, editing: editing) { self.editing = nil }
                }
                VStack(alignment: .leading, spacing: 16) {
                    Text("Tags").font(.title2.bold())
                        TagFlowLayout(spacing: 8) {
                            ForEach(model.tags, id: \.id) { tag in
                                NavigationLink(value: OrganizeDestination.group(tagRoute(tag))) {
                                    Label(tag.name, systemImage: "tag").font(.subheadline)
                                        .padding(.horizontal, 12).padding(.vertical, 9)
                                        .foregroundStyle(.primary)
                                        .background(.quaternary.opacity(0.5), in: .capsule)
                                }
                                .buttonStyle(.plain)
                                .contextMenu {
                                    Button("Rename", systemImage: "pencil") { editing = .rename(tagRoute(tag)) }
                                    Button("Move to Trash", systemImage: "trash", role: .destructive) {
                                        deleting = tagRoute(tag)
                                    }
                                }
                            }
                            Button { editing = .newTag } label: {
                                Label("New Tag", systemImage: "plus").font(.subheadline)
                                    .padding(.horizontal, 12).padding(.vertical, 9)
                                    .background(Color.accentColor.opacity(0.12), in: .capsule)
                            }.buttonStyle(.plain).foregroundStyle(Color.accentColor)
                        }
                    if let editing, !isCollectionEdit(editing) {
                        GroupInlineEditor(model: model, editing: editing) { self.editing = nil }
                    }
                    Text(
                        model.tags.isEmpty
                            ? "No tags yet. Long press a sticker and choose Organize to create a tag."
                            : "Tap a tag to view its stickers. Long press to rename or delete."
                    )
                    .font(.caption).foregroundStyle(.secondary)
                }
            }
            .padding(16)
            .frame(maxWidth: 920, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .background(Color(uiColor: .systemGroupedBackground))
        .navigationTitle("Organize")
        .modifier(
            GroupManagementPresentation(model: model, editing: $editing, deleting: $deleting, presentsEditor: false)
        )
        .refreshable { await model.refresh() }
    }

    private func isCollectionEdit(_ value: GroupEditRequest) -> Bool {
        switch value {
        case .newCollection: true
        case .newTag: false
        case .rename(let route): if case .collection = route.filter { true } else { false }
        }
    }

    private func collectionRoute(_ value: CollectionMetadata) -> GroupRoute {
        GroupRoute(filter: .collection(value.id), generation: value.lifecycle.generation)
    }

    private func tagRoute(_ value: TagMetadata) -> GroupRoute {
        GroupRoute(filter: .tag(value.id), generation: value.lifecycle.generation)
    }
}

/// Wrap intrinsic-width tags while allowing long names to fit the proposed width.
struct TagFlowLayout: Layout {
    let spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        layout(subviews, width: proposal.width ?? 320).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = layout(subviews, width: bounds.width)
        for (index, item) in subviews.enumerated() {
            item.place(
                at: CGPoint(x: bounds.minX + result.points[index].x, y: bounds.minY + result.points[index].y),
                proposal: ProposedViewSize(width: result.sizes[index].width, height: result.sizes[index].height))
        }
    }

    private func layout(_ subviews: Subviews, width: CGFloat) -> (size: CGSize, points: [CGPoint], sizes: [CGSize]) {
        let width = max(1, width)
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var points: [CGPoint] = []
        var sizes: [CGSize] = []
        for item in subviews {
            let ideal = item.sizeThatFits(.unspecified)
            let size = item.sizeThatFits(ProposedViewSize(width: min(ideal.width, width), height: nil))
            if x > 0 && x + size.width > width {
                y += rowHeight + spacing
                x = 0
                rowHeight = 0
            }
            points.append(CGPoint(x: x, y: y))
            sizes.append(size)
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
        return (CGSize(width: width, height: y + rowHeight), points, sizes)
    }
}
