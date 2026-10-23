import CoreTransferable
import Foundation
import MemeDockBridge
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

enum ImportSource {
    case file(URL)
    case photo(PhotosPickerItem)

    var name: String {
        switch self {
        case .file(let url): url.lastPathComponent
        case .photo: String(localized: "Photo")
        }
    }

    @MainActor
    func write(
        to destination: URL, maximumBytes: UInt64, cancellation: ImportCancellation,
        transferProgress: (Progress?) -> Void
    ) async throws -> String {
        switch self {
        case .file(let url):
            try await ImportFileIO.copySelectedFile(
                from: url, to: destination, maximumBytes: maximumBytes, cancellation: cancellation
            )
            return url.lastPathComponent
        case .photo(let item):
            let photo: ImportPhotoFile
            do {
                photo = try await withCheckedThrowingContinuation { continuation in
                    let progress = item.loadTransferable(type: ImportPhotoFile.self) { result in
                        continuation.resume(with: result.flatMap { value in
                            value.map(Result.success) ?? .failure(ImportInputError.unavailable)
                        })
                    }
                    transferProgress(progress)
                    if cancellation.isCancelled { progress.cancel() }
                }
            } catch {
                transferProgress(nil)
                // A provider may use its own cancellation error domain.
                if cancellation.isCancelled { throw CancellationError() }
                throw error
            }
            transferProgress(nil)
            // Always remove our provider copy, including when cancellation won the race.
            do {
                try cancellation.check()
                try await ImportFileIO.copy(
                    from: photo.url, to: destination,
                    maximumBytes: maximumBytes, cancellation: cancellation
                )
            } catch {
                do { try await ImportFileIO.remove(photo.directory) }
                catch let cleanup { throw ImportCleanupError(operation: error, cleanup: cleanup) }
                throw error
            }
            try await ImportFileIO.remove(photo.directory)
            return photo.name
        }
    }
}

private struct ImportPhotoFile: Transferable, Sendable {
    let url: URL
    let directory: URL
    let name: String

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(importedContentType: .image) { received in
            // Provider URLs expire when this closure returns. Own a bounded file copy first.
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent("MemeDockPhoto-\(UUID().uuidString)", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent("input")
            do {
                try await ImportFileIO.copy(
                    from: received.file, to: url,
                    maximumBytes: try defaultResourceConfiguration().maxFileBytes,
                    cancellation: ImportCancellation()
                )
                return Self(url: url, directory: directory, name: received.file.lastPathComponent)
            } catch {
                do { try await ImportFileIO.remove(directory) }
                catch let cleanup { throw ImportCleanupError(operation: error, cleanup: cleanup) }
                throw error
            }
        }
    }
}

/// Nonisolated async functions run blocking file operations away from the main actor
/// under this project's Swift 6 settings (no default-isolation/upcoming-feature overrides).
enum ImportFileIO {
    nonisolated static func removeAbandonedPhotoCopies() async throws {
        let manager = FileManager.default
        let entries = try manager.contentsOfDirectory(
            at: manager.temporaryDirectory, includingPropertiesForKeys: nil
        )
        for url in entries where url.lastPathComponent.hasPrefix("MemeDockPhoto-") {
            try manager.removeItem(at: url)
        }
    }

    nonisolated static func copySelectedFile(
        from source: URL, to destination: URL, maximumBytes: UInt64,
        cancellation: ImportCancellation
    ) async throws {
        let accessed = source.startAccessingSecurityScopedResource()
        defer { if accessed { source.stopAccessingSecurityScopedResource() } }
        let coordination = ImportFileCoordination()
        cancellation.setReadCancellation { coordination.cancel() }
        defer { cancellation.setReadCancellation(nil) }
        try cancellation.check()
        try coordination.read(source) { url in
            try copyFile(from: url, to: destination, maximumBytes: maximumBytes, cancellation: cancellation)
        }
    }

    nonisolated static func copy(
        from source: URL, to destination: URL, maximumBytes: UInt64,
        cancellation: ImportCancellation
    ) async throws {
        try copyFile(from: source, to: destination, maximumBytes: maximumBytes, cancellation: cancellation)
    }

    private nonisolated static func copyFile(
        from source: URL, to destination: URL, maximumBytes: UInt64,
        cancellation: ImportCancellation
    ) throws {
        try cancellation.check()
        try Task.checkCancellation()
        let values = try source.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        guard values.isRegularFile == true else { throw ImportInputError.notAFile }
        if let size = values.fileSize, UInt64(size) > maximumBytes { throw ImportInputError.tooLarge }
        let input = try FileHandle(forReadingFrom: source)
        defer { try? input.close() }
        if !FileManager.default.fileExists(atPath: destination.path) {
            guard FileManager.default.createFile(atPath: destination.path, contents: nil) else {
                throw CocoaError(.fileWriteUnknown)
            }
        }
        let output = try FileHandle(forWritingTo: destination)
        defer { try? output.close() }
        try output.truncate(atOffset: 0)
        var count: UInt64 = 0
        while true {
            try cancellation.check()
            try Task.checkCancellation()
            guard let chunk = try input.read(upToCount: 64 * 1024), !chunk.isEmpty else { break }
            count += UInt64(chunk.count)
            guard count <= maximumBytes else { throw ImportInputError.tooLarge }
            try output.write(contentsOf: chunk)
        }
        try cancellation.check()
        try output.synchronize()
        try output.close()
    }

    nonisolated static func remove(_ url: URL) async throws {
        try FileManager.default.removeItem(at: url)
    }
}

/// Safety: one synchronous reader owns the coordinator on its worker thread.
/// The only cross-thread operation is cancel(), explicitly permitted by Foundation.
/// Keep the underlying object private; replace this wrapper when Foundation exposes
/// a checked Sendable cancellation handle for coordinated reads.
private final class ImportFileCoordination: @unchecked Sendable {
    private let coordinator = NSFileCoordinator()

    func cancel() { coordinator.cancel() }

    func read(_ source: URL, body: (URL) throws -> Void) throws {
        var coordinationError: NSError?
        var result: Result<Void, any Error>?
        coordinator.coordinate(readingItemAt: source, options: .withoutChanges, error: &coordinationError) { url in
            result = Result { try body(url) }
        }
        if let coordinationError { throw coordinationError }
        guard let result else { throw ImportInputError.unavailable }
        try result.get()
    }
}
