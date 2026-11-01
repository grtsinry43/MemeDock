import AppKit
import SwiftUI

/// Bottom-of-window celebratory card for the brand easter egg. It never takes
/// focus and disappears on its own or from the close button.
struct EasterEggToast: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let onClose: () -> Void

    var body: some View {
        let palette = MemeDockPalette(scheme)
        HStack(spacing: 12) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 32, height: 32)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(L10n.text(.eggFoundTitle))
                    .font(MemeDockTypography.rowTitle)
                    .foregroundStyle(palette.text)
                Text(L10n.text(.eggFoundMessage))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.secondary)
            }
            Spacer(minLength: 0)
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .foregroundStyle(palette.secondary)
            }
            .buttonStyle(.plain)
            .help(L10n.text(.close))
            .accessibilityLabel(L10n.text(.close))
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(maxWidth: 420)
        .background(palette.raised, in: RoundedRectangle(cornerRadius: MemeDockGeometry.message))
        .overlay(
            RoundedRectangle(cornerRadius: MemeDockGeometry.message)
                .stroke(palette.separator, lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.16), radius: 12, y: 6)
        .padding(.horizontal, 16)
        .transition(reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity))
    }
}
