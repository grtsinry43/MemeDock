import SwiftUI
import UIKit

struct SystemOutputView: UIViewControllerRepresentable {
    let value: OutputPresentation
    let output: OutputCoordinator
    func makeCoordinator() -> Coordinator { Coordinator(value: value, output: output) }
    func makeUIViewController(context: Context) -> UIViewController {
        let url = URL(fileURLWithPath: value.artifact.path)
        if value.action == .files {
            let picker = UIDocumentPickerViewController(forExporting: [url], asCopy: true)
            picker.delegate = context.coordinator
            return picker
        }
        let controller = UIActivityViewController(activityItems: [url], applicationActivities: nil)
        controller.completionWithItemsHandler = { _, completed, _, error in
            let message = error?.localizedDescription
            Task { @MainActor in await output.complete(value.id, completed: completed, error: message) }
        }
        if let popover = controller.popoverPresentationController {
            popover.sourceView = controller.view
            popover.sourceRect = CGRect(
                x: controller.view.bounds.midX, y: controller.view.bounds.midY, width: 1, height: 1)
        }
        return controller
    }
    func updateUIViewController(_ controller: UIViewController, context: Context) {}
    @MainActor final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let value: OutputPresentation
        let output: OutputCoordinator
        init(value: OutputPresentation, output: OutputCoordinator) {
            self.value = value
            self.output = output
        }
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            Task { await output.complete(value.id, completed: !urls.isEmpty, error: nil) }
        }
        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
            Task { await output.complete(value.id, completed: false, error: nil) }
        }
    }
}
