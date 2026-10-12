import Foundation

/// Storage locations owned by this app. The three roots stay siblings so
/// clearing the cache or the export shelf never touches originals, mirroring
/// the Linux layout (`sticker-library`, `sticker-exports`).
struct AppPaths {
    let data: URL
    let cache: URL
    let export: URL

    static func standard() -> AppPaths {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        let root = support.appendingPathComponent("MemeDock", isDirectory: true)
        return AppPaths(
            data: root.appendingPathComponent("sticker-library", isDirectory: true),
            cache: caches.appendingPathComponent("MemeDock", isDirectory: true),
            export: root.appendingPathComponent("sticker-exports", isDirectory: true)
        )
    }

    /// The library creates its own tree; the parent must exist so a fresh
    /// install never fails on a missing Application Support directory.
    func prepare() throws {
        for url in [data.deletingLastPathComponent(), export.deletingLastPathComponent(), cache] {
            try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        }
    }
}
