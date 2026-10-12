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
