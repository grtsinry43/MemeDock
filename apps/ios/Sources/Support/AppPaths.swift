import Foundation
import MemeDockBridge

struct AppPaths: Sendable {
    let data: URL
    let cache: URL
    let export: URL

    static func standard() throws -> AppPaths {
        let manager = FileManager.default
        let support = try manager.url(
            for: .applicationSupportDirectory, in: .userDomainMask,
            appropriateFor: nil, create: true
        ).appendingPathComponent("MemeDock", isDirectory: true)
        let caches = try manager.url(
            for: .cachesDirectory, in: .userDomainMask,
            appropriateFor: nil, create: true
        )
        return AppPaths(
            data: support.appendingPathComponent("Library", isDirectory: true),
            cache: caches.appendingPathComponent("MemeDock", isDirectory: true),
            export: caches.appendingPathComponent("MemeDockExports", isDirectory: true)
        )
    }

    func configuration() throws -> LibraryConfiguration {
        LibraryConfiguration(
            dataDir: data.path, cacheDir: cache.path, exportDir: export.path,
            limits: try defaultResourceConfiguration()
        )
    }
}
