import SwiftUI

private struct DependencyLicense: Decodable, Identifiable, Sendable {
    let name: String
    let version: String
    let license: String?
    let repository: String?
    var id: String { name + ":" + version }
}

private actor LicenseReader {
    static let shared = LicenseReader()
    func load() throws -> [DependencyLicense] {
        guard let url = Bundle.main.url(forResource: "ThirdPartyLicenses", withExtension: "json") else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try JSONDecoder().decode([DependencyLicense].self, from: Data(contentsOf: url))
    }
}

struct LicensesView: View {
    @State private var dependencies: [DependencyLicense] = []
    @State private var error: String?
    var body: some View {
        List {
            Section("MemeDock") {
                Text("MIT License · Copyright © 2026 grtsinry43")
                Link("Source Code", destination: URL(string: "https://github.com/grtsinry43/MemeDock")!)
            }
            Section("Third-Party Software") {
                ForEach(dependencies) { item in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(item.name).font(.headline)
                            Spacer()
                            Text(item.version).foregroundStyle(.secondary)
                        }
                        Text(item.license ?? "").font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                        if let address = item.repository, let url = URL(string: address),
                            ["https", "http"].contains(url.scheme)
                        {
                            Link("Project and License", destination: url).font(.subheadline)
                        }
                    }.padding(.vertical, 4)
                }
            }
            if let error {
                Text(error).foregroundStyle(.red)
                Button("Retry") { Task { await load() } }
            }
        }
        .navigationTitle("Open Source Licenses").navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }
    private func load() async {
        do {
            dependencies = try await LicenseReader.shared.load()
            error = nil
        } catch { self.error = error.localizedDescription }
    }
}
