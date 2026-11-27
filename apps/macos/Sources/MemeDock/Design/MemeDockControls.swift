import AppKit
import SwiftUI

/// Window-scoped native materials; never attach a separate blur to every tile.
struct MemeDockGlass: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    var sidebar = false

    var body: some View {
        let palette = MemeDockPalette(scheme)
        Group {
            if reduceTransparency {
                palette.canvas
            } else {
                MaterialView(sidebar: sidebar)
                    .overlay(palette.canvas.opacity(0.72))
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private struct MaterialView: NSViewRepresentable {
        let sidebar: Bool

        func makeNSView(context: Context) -> NSVisualEffectView {
            let view = NSVisualEffectView()
            configure(view)
            return view
        }

        func updateNSView(_ view: NSVisualEffectView, context: Context) {
            configure(view)
        }

        private func configure(_ view: NSVisualEffectView) {
            view.material = sidebar ? .sidebar : .headerView
            view.blendingMode = sidebar ? .behindWindow : .withinWindow
            view.state = .followsWindowActiveState
        }
    }
}

struct MemeDockButtonStyle: ButtonStyle {
    enum Kind: Equatable { case primary, secondary, quiet, destructive }
    var kind: Kind = .secondary

    func makeBody(configuration: Configuration) -> some View {
        StyledButton(configuration: configuration, kind: kind)
    }

    private struct StyledButton: View {
        @Environment(\.colorScheme) private var scheme
        @Environment(\.isEnabled) private var enabled
        @Environment(\.accessibilityReduceMotion) private var reduceMotion
        @State private var hovering = false
        let configuration: ButtonStyle.Configuration
        let kind: Kind

        var body: some View {
            let palette = MemeDockPalette(scheme)
            configuration.label
                .font(MemeDockTypography.control)
                .padding(.horizontal, kind == .quiet ? 8 : 12)
                .frame(minHeight: MemeDockGeometry.controlHeight)
                .foregroundStyle(foreground(palette))
                .background(background(palette), in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
                .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
                .opacity(enabled ? 1 : 0.38)
                .scaleEffect(configuration.isPressed && !reduceMotion ? 0.98 : 1)
                .onHover { hovering = $0 }
                .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: configuration.isPressed)
                .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
        }

        private func foreground(_ palette: MemeDockPalette) -> Color {
            if configuration.role == .destructive { return palette.error }
            switch kind {
            case .primary: return palette.raised
            case .destructive: return palette.error
            case .secondary, .quiet: return palette.text
            }
        }

        private func background(_ palette: MemeDockPalette) -> Color {
            switch kind {
            case .primary: return palette.text.opacity(configuration.isPressed ? 0.86 : 1)
            case .quiet, .destructive:
                return enabled && (hovering || configuration.isPressed) ? palette.input : .clear
            case .secondary:
                return enabled && (hovering || configuration.isPressed) ? palette.pressed : palette.input
            }
        }
    }
}

struct MemeDockFieldStyle: TextFieldStyle {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var enabled
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var focused = false

    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration
            .textFieldStyle(.plain)
            .font(MemeDockTypography.body)
            .tint(MemeDockPalette(scheme).accent)
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(MemeDockPalette(scheme).input,
                        in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            .overlay {
                RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                    .strokeBorder(
                        enabled && focused ? MemeDockPalette(scheme).text.opacity(0.3) : .clear,
                        lineWidth: 1
                    )
            }
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: focused)
    }
}

/// Top-left brand title with a refined mode-switcher popover that animates
/// between "MemeDock" and "Meme Creator".
struct MemeDockBrandSwitcher: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Binding var isCreator: Bool
    var compact = false
    @State private var hovering = false
    @State private var open = false

    private var switchAnimation: Animation? {
        reduceMotion ? nil : .spring(response: 0.38, dampingFraction: 0.88)
    }

    var body: some View {
        let palette = MemeDockPalette(scheme)
        Button {
            open.toggle()
        } label: {
            HStack(spacing: 0) {
                Text("Meme")
                    .font(compact ? MemeDockTypography.rowTitle : MemeDockTypography.sectionTitle)
                    .foregroundStyle(palette.text)
                ZStack(alignment: .leading) {
                    if isCreator {
                        Text(" Creator")
                            .font(compact ? MemeDockTypography.rowTitle : MemeDockTypography.sectionTitle)
                            .foregroundStyle(palette.text)
                            .transition(wordTransition(goingToCreator: true))
                    } else {
                        Text("Dock")
                            .font(compact ? MemeDockTypography.rowTitle : MemeDockTypography.sectionTitle)
                            .foregroundStyle(palette.text)
                            .transition(wordTransition(goingToCreator: false))
                    }
                }
                .clipped()
                Image(systemName: "chevron.down")
                    .font(.system(size: compact ? 9 : 10, weight: .semibold))
                    .foregroundStyle(hovering || open ? palette.text : palette.secondary)
                    .rotationEffect(.degrees(open ? 180 : 0))
                    .padding(.leading, 6)
            }
            .padding(.horizontal, compact ? 8 : 10)
            .padding(.vertical, compact ? 5 : 6)
            .background(
                hovering || open ? palette.input : .clear,
                in: RoundedRectangle(cornerRadius: MemeDockGeometry.control)
            )
            .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: open)
            .animation(switchAnimation, value: isCreator)
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .popover(isPresented: $open, arrowEdge: .bottom) {
            modeMenuContent(palette)
        }
        .help(L10n.text(.creatorSwitchMode))
        .accessibilityLabel(isCreator ? "Meme Creator" : "MemeDock")
        .accessibilityHint(L10n.text(.creatorSwitchMode))
    }

    private func modeMenuContent(_ palette: MemeDockPalette) -> some View {
        VStack(spacing: 4) {
            ModeRow(
                title: "MemeDock",
                subtitle: L10n.text(.modeDockSubtitle),
                symbol: "square.grid.2x2",
                selected: !isCreator
            ) {
                select(creator: false)
            }
            ModeRow(
                title: "Meme Creator",
                subtitle: L10n.text(.modeCreatorSubtitle),
                symbol: "film.stack",
                selected: isCreator
            ) {
                select(creator: true)
            }
        }
        .padding(8)
        .frame(width: 248)
        .background(palette.raised)
        .presentationCornerRadius(MemeDockGeometry.dialog)
    }

    private func select(creator: Bool) {
        open = false
        guard isCreator != creator else { return }
        withAnimation(switchAnimation) {
            isCreator = creator
        }
    }

    private func wordTransition(goingToCreator: Bool) -> AnyTransition {
        if reduceMotion { return .opacity }
        let enterY: CGFloat = goingToCreator ? 14 : -14
        let exitY: CGFloat = goingToCreator ? -14 : 14
        return .asymmetric(
            insertion: .offset(y: enterY).combined(with: .opacity),
            removal: .offset(y: exitY).combined(with: .opacity)
        )
    }

    private struct ModeRow: View {
        @Environment(\.colorScheme) private var scheme
        @Environment(\.accessibilityReduceMotion) private var reduceMotion
        let title: String
        let subtitle: String
        let symbol: String
        let selected: Bool
        let action: () -> Void
        @State private var hovering = false

        var body: some View {
            let palette = MemeDockPalette(scheme)
            Button(action: action) {
                HStack(spacing: 10) {
                    Image(systemName: symbol)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(selected ? palette.raised : palette.text)
                        .frame(width: 30, height: 30)
                        .background(
                            selected ? palette.text : palette.input,
                            in: RoundedRectangle(cornerRadius: MemeDockGeometry.badge + 2)
                        )
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title)
                            .font(MemeDockTypography.control)
                            .foregroundStyle(palette.text)
                        Text(subtitle)
                            .font(MemeDockTypography.caption)
                            .foregroundStyle(palette.secondary)
                            .lineLimit(1)
                    }
                    Spacer(minLength: 4)
                    if selected {
                        Image(systemName: "checkmark")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(palette.text)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(
                    hovering || selected ? palette.input : .clear,
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                )
                .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
                .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
            }
            .buttonStyle(.plain)
            .onHover { hovering = $0 }
            .accessibilityAddTraits(selected ? .isSelected : [])
        }
    }
}

/// Neutral-contrast segmented control styled to match MemeDock's surface tokens.
struct MemeDockSegmentedControl<Selection: Hashable>: View {
    struct Option: Identifiable {
        let value: Selection
        let title: String
        var badge: String? = nil
        var dot: Bool = false
        var id: Selection { value }
    }

    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Binding var selection: Selection
    let options: [Option]
    @Namespace private var thumbNamespace

    var body: some View {
        let palette = MemeDockPalette(scheme)
        HStack(spacing: 2) {
            ForEach(options) { option in
                let active = selection == option.value
                Button {
                    withAnimation(reduceMotion ? nil : MemeDockMotion.feedback) {
                        selection = option.value
                    }
                } label: {
                    HStack(spacing: 5) {
                        Text(option.title)
                            .font(MemeDockTypography.control)
                        if let badge = option.badge {
                            Text(badge)
                                .font(MemeDockTypography.caption)
                                .monospacedDigit()
                                .opacity(active ? 0.78 : 0.7)
                        }
                        if option.dot {
                            Circle()
                                .fill(active ? palette.raised : palette.star)
                                .frame(width: 5, height: 5)
                        }
                    }
                    .foregroundStyle(active ? palette.raised : palette.secondary)
                    .padding(.horizontal, 12)
                    .frame(height: 28)
                    .background {
                        if active {
                            RoundedRectangle(cornerRadius: MemeDockGeometry.control - 2)
                                .fill(palette.text)
                                .matchedGeometryEffect(id: "thumb", in: thumbNamespace)
                        }
                    }
                    .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control - 2))
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(active ? .isSelected : [])
            }
        }
        .padding(3)
        .background(palette.input, in: RoundedRectangle(cornerRadius: MemeDockGeometry.control))
    }
}

/// Compact selectable chip matching MemeDock's neutral-contrast controls.
struct MemeDockChoiceChip: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var enabled
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let title: String
    let selected: Bool
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        let palette = MemeDockPalette(scheme)
        Button(action: action) {
            Text(title)
                .font(MemeDockTypography.caption)
                .fontWeight(selected ? .medium : .regular)
                .monospacedDigit()
                .lineLimit(1)
                .foregroundStyle(selected ? palette.raised : palette.text)
                .padding(.horizontal, 10)
                .frame(minHeight: 28)
                .frame(maxWidth: .infinity)
                .background(
                    selected ? palette.text : (hovering && enabled ? palette.pressed : palette.input),
                    in: RoundedRectangle(cornerRadius: MemeDockGeometry.badge + 2)
                )
                .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.badge + 2))
        }
        .buttonStyle(.plain)
        .opacity(enabled ? 1 : 0.4)
        .onHover { hovering = $0 }
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
        .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: selected)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}
