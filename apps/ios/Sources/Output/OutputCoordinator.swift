import Foundation
import MemeDockBridge
import Observation
import Photos
import UIKit
import UniformTypeIdentifiers

struct OutputPresentation: Identifiable {
    let id = UUID()
    let action: OutputAction
    let sticker: String
    let artifact: ExportArtifact
    let lease: ArtifactLeaseHandle
}

actor OutputFileIO {
    static let shared = OutputFileIO()
    func read(_ artifact: ExportArtifact) throws -> Data {
        let maximum = try defaultResourceConfiguration().maxFileBytes
        guard artifact.byteSize <= maximum else { throw PreviewError.resourceLimit }
        let data = try Data(contentsOf: URL(fileURLWithPath: artifact.path), options: .mappedIfSafe)
        guard data.count == artifact.byteSize else { throw PreviewError.changedOriginal }
        return data
    }
}

@MainActor @Observable
final class OutputCoordinator {
    private(set) var isBusy = false
    private(set) var stickerID: String?
    private(set) var saving = false
    var presentation: OutputPresentation?
    @ObservationIgnored private let library: LibraryHandle
    @ObservationIgnored private let feedback: AppFeedback
    @ObservationIgnored private var task: ExportTask?
    @ObservationIgnored private var work: Task<Void, Never>?
    @ObservationIgnored private var activePresentation: OutputPresentation?
    @ObservationIgnored private var closed = false

    init(library: LibraryHandle, feedback: AppFeedback) {
        self.library = library
        self.feedback = feedback
    }

    func start(_ action: OutputAction, sticker: String, preset: OutputPreset, firstFrame: Bool = false) {
        guard !closed, !isBusy else { return }
        isBusy = true
        saving = action == .photos || action == .files
        stickerID = sticker
        work = Task {
            do {
                let options = ExportOptions(
                    preset: firstFrame ? .compatiblePng : preset.bridge,
                    maxEdge: nil, backgroundRgba: nil, stripMetadata: true,
                    animation: firstFrame ? .firstFrame : .preserve)
                let exporting = try library.export(id: sticker, options: options)
                task = exporting
                let lease = try await exporting.awaitResult()
                task = nil
                guard !closed, !Task.isCancelled else {
                    finishState()
                    return
                }
                let artifact = try lease.metadata()
                switch action {
                case .share, .files:
                    _ = try await library.prepareHandoff(lease: lease).awaitResult()
                    let value = OutputPresentation(action: action, sticker: sticker, artifact: artifact, lease: lease)
                    activePresentation = value
                    presentation = value
                case .copy:
                    let data = try await OutputFileIO.shared.read(artifact)
                    let uti = UTType(mimeType: artifact.mime)?.identifier ?? UTType.data.identifier
                    UIPasteboard.general.setItems([[uti: data]])
                    await record(sticker, action: .copyImage, success: String(localized: "Copied to Clipboard"))
                    finishState()
                case .photos:
                    let permission = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
                    guard permission == .authorized || permission == .limited else {
                        throw BridgeError.Failure(code: .permissionDenied, detail: "")
                    }
                    let url = URL(fileURLWithPath: artifact.path)
                    try await PHPhotoLibrary.shared().performChanges {
                        let request = PHAssetCreationRequest.forAsset()
                        request.addResource(with: .photo, fileURL: url, options: nil)
                    }
                    await record(sticker, action: .exportSaved, success: String(localized: "Saved to Photos"))
                    finishState()
                }
            } catch {
                if !closed, !Task.isCancelled, !Self.cancelled(error) {
                    feedback.post(BridgeErrors.outputDescription(error), kind: .error)
                }
                finishState()
            }
        }
    }

    func cancel() { _ = task?.cancel() }

    func complete(_ id: UUID, completed: Bool, error: String?) async {
        guard let current = activePresentation, current.id == id else { return }
        activePresentation = nil
        presentation = nil
        if let error {
            feedback.post(error, kind: .error)
        } else if completed {
            await record(
                current.sticker, action: current.action == .share ? .shareLaunched : .exportSaved,
                success: current.action == .files ? String(localized: "File Saved") : nil)
        }
        finishState()
    }

    func presentationDismissed() async {
        guard let current = activePresentation else { return }
        await complete(current.id, completed: false, error: nil)
    }

    func close() async {
        closed = true
        _ = task?.cancel()
        await work?.value
        activePresentation = nil
        presentation = nil
        finishState()
    }

    private func record(_ id: String, action: UsageAction, success: String?) async {
        do {
            _ = try await library.recordUse(id: id, action: action).awaitResult()
            if let success { feedback.post(success) }
        } catch {
            feedback.post(
                String(localized: "The image was delivered, but recent usage couldn’t be updated."), kind: .warning)
        }
    }
    private func finishState() {
        isBusy = false
        saving = false
        stickerID = nil
        task = nil
    }
    private static func cancelled(_ error: any Error) -> Bool {
        if case BridgeError.Failure(let code, _) = error { return code == .cancelled }
        return error is CancellationError
    }
}
