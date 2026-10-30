import Accessibility
import Observation
import SwiftUI

@MainActor @Observable
final class AppFeedback {
    enum Kind { case success, warning, error }
    enum Action: Equatable {
        case viewTrash
        case viewImported(String)
        case viewImport
    }
    struct Message: Identifiable {
        let id = UUID()
        let text: String
        let kind: Kind
        let action: Action?
    }
    private(set) var current: Message?
    @ObservationIgnored private var pending: [Message] = []
    @ObservationIgnored private var timer: Task<Void, Never>?

    func post(_ text: String, kind: Kind = .success, action: Action? = nil) {
        pending.append(Message(text: text, kind: kind, action: action))
        advance()
    }
    func dismiss() {
        timer?.cancel()
        current = nil
        advance()
    }
    private func advance() {
        guard current == nil, !pending.isEmpty else { return }
        let message = pending.removeFirst()
        current = message
        AccessibilityNotification.Announcement(message.text).post()
        timer = Task { [weak self] in
            do { try await Task.sleep(for: .seconds(5)) } catch { return }
            self?.dismiss()
        }
    }
}

/// A transient capsule under the status bar, clear of the bottom bars that hold sending actions.
struct FeedbackBanner: View {
    let feedback: AppFeedback
    let perform: (AppFeedback.Action) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @GestureState private var drag: CGFloat = 0

    var body: some View {
        VStack {
            if let message = feedback.current {
                capsule(message)
                    .id(message.id)
                    .transition(reduceMotion ? .opacity : .move(edge: .top).combined(with: .opacity))
            }
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .snappy, value: feedback.current?.id)
        .sensoryFeedback(trigger: feedback.current?.id) {
            switch feedback.current?.kind {
            case .success: .success
            case .warning: .warning
            case .error: .error
            case nil: nil
            }
        }
    }

    private func capsule(_ message: AppFeedback.Message) -> some View {
        HStack(spacing: 10) {
            Image(systemName: icon(message.kind)).foregroundStyle(tint(message.kind)).imageScale(.large)
                .accessibilityHidden(true)
            Text(message.text).font(.subheadline.weight(.medium)).lineLimit(3)
                .accessibilityAction(named: Text("Dismiss")) { feedback.dismiss() }
            if let action = message.action {
                Button("View") {
                    feedback.dismiss()
                    perform(action)
                }
                .font(.subheadline.weight(.semibold))
                .buttonStyle(.borderless)
            }
        }
        .padding(.leading, 16).padding(.trailing, message.action == nil ? 18 : 16).padding(.vertical, 12)
        .background(.regularMaterial, in: .capsule)
        .shadow(color: .black.opacity(0.12), radius: 12, y: 4)
        .padding(.horizontal, 16).padding(.top, 4)
        .frame(maxWidth: 560)
        .offset(y: min(0, drag))
        .gesture(
            DragGesture(minimumDistance: 8)
                .updating($drag) { value, state, _ in state = value.translation.height }
                .onEnded { value in if value.translation.height < -24 { feedback.dismiss() } }
        )
        .accessibilityElement(children: .contain)
    }

    private func icon(_ kind: AppFeedback.Kind) -> String {
        switch kind {
        case .success: "checkmark.circle.fill"
        case .warning: "exclamationmark.triangle.fill"
        case .error: "xmark.octagon.fill"
        }
    }

    private func tint(_ kind: AppFeedback.Kind) -> Color {
        switch kind {
        case .success: .green
        case .warning: .orange
        case .error: .red
        }
    }
}
