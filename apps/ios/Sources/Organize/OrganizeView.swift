import SwiftUI

struct OrganizeView: View {
    let model: OrganizeModel

    var body: some View {
        List {
            if let error = model.error {
                Section {
                    Text(error).foregroundStyle(.secondary)
                    Button("Retry") { Task { await model.refresh() } }
                }
            }
            if model.isLoading, model.collections.isEmpty, model.tags.isEmpty {
                ProgressView()
            } else {
                Section("Collections") {
                    if model.collections.isEmpty {
                        Text("No Collections").foregroundStyle(.secondary)
                    }
                    ForEach(model.collections, id: \.collection.id) { summary in
                        NavigationLink(value: OrganizeDestination.group(GroupRoute(
                            filter: .collection(summary.collection.id), generation: summary.collection.lifecycle.generation
                        ))) {
                            HStack {
                                Label(summary.collection.name, systemImage: "folder").foregroundStyle(.primary)
                                Spacer()
                                Text(summary.count.formatted()).foregroundStyle(.secondary)
                            }
                        }
                        .accessibilityHint("Open Collection")
                    }
                }
                Section("Tags") {
                    if model.tags.isEmpty { Text("No Tags").foregroundStyle(.secondary) }
                    ForEach(model.tags, id: \.id) { tag in
                        NavigationLink(value: OrganizeDestination.group(GroupRoute(
                            filter: .tag(tag.id), generation: tag.lifecycle.generation
                        ))) {
                            Label(tag.name, systemImage: "tag")
                        }
                        .accessibilityHint("Open Tag")
                    }
                }
            }
        }
        .navigationTitle("Organize")
        .refreshable { await model.refresh() }
    }
}
