import SwiftUI

struct MemeDockDisclosureStyle: DisclosureGroupStyle {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                withAnimation(reduceMotion ? nil : MemeDockMotion.feedback) {
                    configuration.isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 10, weight: .medium))
                        .rotationEffect(.degrees(configuration.isExpanded ? 90 : 0))
                    configuration.label
                    Spacer(minLength: 0)
                }
                .frame(maxWidth: .infinity, minHeight: MemeDockGeometry.controlHeight, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityValue(L10n.text(configuration.isExpanded ? .expanded : .collapsed))
            if configuration.isExpanded { configuration.content }
        }
    }
}
