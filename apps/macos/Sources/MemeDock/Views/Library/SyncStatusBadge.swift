import MemeDockBridge
import SwiftUI

/// Refined, unobtrusive sync status indicator with soft blur-and-glide state
/// transitions, subtle motion while syncing, and a quick-action popover.
struct SyncStatusBadge: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ObservedObject var sync: SyncCoordinator
    var compact = false

    @State private var hovering = false
    @State private var open = false
    @State private var spinning = false
    @State private var recentlySynced = false
    @State private var settleWork: Task<Void, Never>?

    private enum Phase: Equatable {
        case hidden
        case syncing(downloading: UInt64)
        case synced
        case pending(count: UInt64)
        case paused
        case issue
        case idle
    }

    private var phase: Phase {
        if sync.error != nil { return .issue }
        guard let state = sync.state, state.connected else { return .hidden }
        if sync.needsAuthorization || state.error != nil || state.blocked > 0 {
            return .issue
        }
        if sync.busy || state.downloading > 0 {
            return .syncing(downloading: state.downloading)
        }
        if state.paused { return .paused }
        if state.pending > 0 { return .pending(count: state.pending) }
        if recentlySynced { return .synced }
        return .idle
    }

    var body: some View {
        let current = phase
        Group {
            if current != .hidden {
                pill(for: current)
                    .transition(statusTransition)
            }
        }
        .animation(reduceMotion ? nil : .spring(response: 0.42, dampingFraction: 0.86), value: current)
        .onChange(of: sync.busy) { wasBusy, isBusy in
            if isBusy {
                settleWork?.cancel()
                recentlySynced = false
            } else if wasBusy, sync.error == nil, sync.state?.error == nil, (sync.state?.blocked ?? 0) == 0 {
                recentlySynced = true
                settleWork?.cancel()
                settleWork = Task { @MainActor in
                    try? await Task.sleep(for: .seconds(2.4))
                    guard !Task.isCancelled else { return }
                    recentlySynced = false
                }
            }
        }
        .onDisappear { settleWork?.cancel() }
    }

    private func pill(for current: Phase) -> some View {
        let palette = MemeDockPalette(scheme)
        let isActive: Bool = {
            switch current {
            case .syncing, .synced, .issue: return true
            default: return false
            }
        }()

        return Button {
            open.toggle()
        } label: {
            HStack(spacing: 8) {
                indicatorIcon(for: current, palette: palette)
                    .frame(width: 16, height: 16)

                if !compact {
                    ZStack(alignment: .leading) {
                        statusText(for: current, palette: palette)
                            .id(statusKey(for: current))
                            .transition(statusTransition)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .clipped()
                }
            }
            .padding(.horizontal, compact ? 8 : 10)
            .frame(height: compact ? MemeDockGeometry.controlHeight : 34)
            .background {
                RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                    .fill(backgroundColor(palette: palette, active: isActive))
                    .overlay {
                        RoundedRectangle(cornerRadius: MemeDockGeometry.control)
                            .strokeBorder(
                                hovering || open || isActive ? palette.separator : .clear,
                                lineWidth: 1
                            )
                    }
            }
            .opacity(hovering || open || isActive ? 1 : 0.72)
            .contentShape(RoundedRectangle(cornerRadius: MemeDockGeometry.control))
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: hovering)
            .animation(reduceMotion ? nil : MemeDockMotion.feedback, value: open)
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .popover(isPresented: $open, arrowEdge: compact ? .bottom : .trailing) {
            popoverContent(palette: palette)
        }
        .help(accessibilityText(for: current))
        .accessibilityLabel(accessibilityText(for: current))
    }

    @ViewBuilder
    private func indicatorIcon(for current: Phase, palette: MemeDockPalette) -> some View {
        switch current {
        case .syncing:
            Image(systemName: "arrow.triangle.2.circlepath")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(palette.text)
                .rotationEffect(.degrees(spinning && !reduceMotion ? 360 : 0))
                .onAppear {
                    guard !reduceMotion else { return }
                    spinning = false
                    withAnimation(.linear(duration: 1.35).repeatForever(autoreverses: false)) {
                        spinning = true
                    }
                }
                .onDisappear { spinning = false }
                .transition(statusTransition)
        case .synced:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(palette.success)
                .transition(statusTransition)
        case .pending:
            Circle()
                .fill(palette.accent)
                .frame(width: 7, height: 7)
                .shadow(color: palette.accent.opacity(0.45), radius: 4)
                .transition(statusTransition)
        case .paused:
            Image(systemName: "pause.circle")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(palette.secondary)
                .transition(statusTransition)
        case .issue:
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(palette.error)
                .transition(statusTransition)
        case .idle:
            Circle()
                .fill(palette.success.opacity(0.85))
                .frame(width: 6, height: 6)
                .transition(statusTransition)
        case .hidden:
            EmptyView()
        }
    }

    @ViewBuilder
    private func statusText(for current: Phase, palette: MemeDockPalette) -> some View {
        switch current {
        case let .syncing(downloading):
            HStack(spacing: 4) {
                Text(L10n.text(.syncRunning))
                    .font(MemeDockTypography.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(palette.text)
                if downloading > 0 {
                    Text("· \(downloading)")
                        .font(MemeDockTypography.caption)
                        .monospacedDigit()
                        .foregroundStyle(palette.secondary)
                }
            }
            .lineLimit(1)
        case .synced:
            Text(L10n.text(.syncReady))
                .font(MemeDockTypography.caption)
                .fontWeight(.medium)
                .foregroundStyle(palette.text)
                .lineLimit(1)
        case let .pending(count):
            Text("\(L10n.text(.syncPending)) · \(count)")
                .font(MemeDockTypography.caption)
                .monospacedDigit()
                .foregroundStyle(palette.text)
                .lineLimit(1)
        case .paused:
            Text(L10n.text(.syncPaused))
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .lineLimit(1)
        case .issue:
            Text(L10n.text(.syncFailed))
                .font(MemeDockTypography.caption)
                .fontWeight(.medium)
                .foregroundStyle(palette.error)
                .lineLimit(1)
        case .idle:
            if let time = sync.state?.lastSuccessMs {
                HStack(spacing: 4) {
                    Text(L10n.text(.syncReady))
                    Text("·")
                        .opacity(0.5)
                    Text(Date(timeIntervalSince1970: Double(time) / 1000), style: .relative)
                }
                .font(MemeDockTypography.caption)
                .foregroundStyle(palette.secondary)
                .lineLimit(1)
            } else {
                Text(L10n.text(.syncReady))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.secondary)
                    .lineLimit(1)
            }
        case .hidden:
            EmptyView()
        }
    }

    private func popoverContent(palette: MemeDockPalette) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                indicatorIcon(for: phase, palette: palette)
                Text(accessibilityText(for: phase))
                    .font(MemeDockTypography.control)
                    .foregroundStyle(palette.text)
                Spacer(minLength: 8)
            }

            if let state = sync.state {
                VStack(alignment: .leading, spacing: 6) {
                    if let time = state.lastSuccessMs {
                        HStack {
                            Text(L10n.text(.syncLastSuccess))
                                .foregroundStyle(palette.secondary)
                            Spacer()
                            Text(Date(timeIntervalSince1970: Double(time) / 1000), style: .relative)
                                .foregroundStyle(palette.text)
                                .monospacedDigit()
                        }
                    }
                    if state.pending > 0 {
                        statRow(L10n.text(.syncPending), value: "\(state.pending)", palette: palette)
                    }
                    if state.downloading > 0 {
                        statRow(L10n.text(.syncDownloading), value: "\(state.downloading)", palette: palette)
                    }
                    if state.blocked > 0 {
                        statRow(L10n.text(.syncBlocked), value: "\(state.blocked)", palette: palette, accent: palette.error)
                    }
                }
                .font(MemeDockTypography.caption)
            }

            if let error = sync.error {
                Text(error)
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(palette.error)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: 8) {
                if let state = sync.state, state.connected, sync.hasCredential {
                    Button(L10n.text(state.paused ? .syncResume : .syncNow)) {
                        open = false
                        Task {
                            if state.paused {
                                await sync.setPaused(false)
                            } else {
                                await sync.synchronizeNow()
                            }
                        }
                    }
                    .buttonStyle(MemeDockButtonStyle(kind: .primary))
                    .disabled(sync.busy)
                }

                SettingsLink {
                    Text(L10n.text(.settings))
                }
                .buttonStyle(MemeDockButtonStyle(kind: .secondary))
            }
        }
        .padding(14)
        .frame(width: 248)
        .background(palette.raised)
        .presentationCornerRadius(MemeDockGeometry.dialog)
    }

    private func statRow(_ label: String, value: String, palette: MemeDockPalette, accent: Color? = nil) -> some View {
        HStack {
            Text(label).foregroundStyle(palette.secondary)
            Spacer()
            Text(value).foregroundStyle(accent ?? palette.text).monospacedDigit()
        }
    }

    private func backgroundColor(palette: MemeDockPalette, active: Bool) -> Color {
        if hovering || open { return palette.input }
        if active { return palette.input.opacity(0.65) }
        return .clear
    }

    private func statusKey(for current: Phase) -> String {
        switch current {
        case .hidden: return "hidden"
        case let .syncing(d): return "syncing-\(d > 0)"
        case .synced: return "synced"
        case let .pending(c): return "pending-\(c)"
        case .paused: return "paused"
        case .issue: return "issue"
        case .idle: return "idle"
        }
    }

    private func accessibilityText(for current: Phase) -> String {
        switch current {
        case .syncing: return L10n.text(.syncRunning)
        case .synced, .idle: return L10n.text(.syncReady)
        case .pending: return L10n.text(.syncIncomplete)
        case .paused: return L10n.text(.syncPaused)
        case .issue: return L10n.text(.syncFailed)
        case .hidden: return ""
        }
    }

    private var statusTransition: AnyTransition {
        if reduceMotion { return .opacity }
        return AnyTransition(SoftBlurGlideTransition())
    }
}

/// Soft blur + subtle vertical glide + fade transition for status changes.
private struct SoftBlurGlideTransition: Transition {
    func body(content: Content, phase: TransitionPhase) -> some View {
        let offsetY: CGFloat
        switch phase {
        case .willAppear: offsetY = 6
        case .identity: offsetY = 0
        case .didDisappear: offsetY = -6
        }
        return content
            .offset(y: offsetY)
            .blur(radius: phase.isIdentity ? 0 : 4)
            .opacity(phase.isIdentity ? 1 : 0)
    }
}
