import SwiftUI

/// A compact overlay whose height follows its message rather than reserving
/// an empty row. Full error details remain available in the help tooltip.
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
            VStack(alignment: .leading, spacing: 6) {
                Text(message)
                    .lineLimit(4)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .help(message)
                if let actionTitle, let action {
                    Button(actionTitle, action: action).buttonStyle(.plain)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
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
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(.regularMaterial,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
    }
}
