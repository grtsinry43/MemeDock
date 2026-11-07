import SwiftUI

/// A bounded message surface: long errors scroll without moving the close button
/// or changing the size of the containing panel.
struct FeedbackBanner: View {
    let message: String
    let isFailure: Bool
    let dismiss: () -> Void
    var actionTitle: String? = nil
    var action: (() -> Void)? = nil
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: isFailure ? "exclamationmark.circle" : "checkmark.circle")
                .frame(width: 18, height: 24)
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(message)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if let actionTitle, let action {
                        Button(actionTitle, action: action).buttonStyle(.plain)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            Button(action: dismiss) {
                Image(systemName: "xmark")
                    .frame(width: 24, height: 24)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.text(.close))
        }
        .font(MemeDockTypography.caption)
        .foregroundStyle(isFailure ? MemeDockPalette(scheme).error : MemeDockPalette(scheme).secondary)
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.regularMaterial,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
    }
}
