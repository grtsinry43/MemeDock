import AppKit
import SwiftUI

/// The grouped form every settings page is built on. The rows and controls
/// are the platform's own, so labels sit on the left and controls line up on
/// the right; only the type and the page colour come from MemeDock.
struct SettingsForm<Content: View>: View {
    @Environment(\.colorScheme) private var scheme
    @ViewBuilder let content: Content

    var body: some View {
        Form { content }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
            .background(MemeDockPalette(scheme).canvas)
            .font(MemeDockTypography.body)
    }
}

/// A row title with the sentence that says what it covers.
struct SettingsLabel: View {
    @Environment(\.colorScheme) private var scheme
    let title: String
    var detail: String?

    init(_ title: L10nKey, detail: L10nKey? = nil) {
        self.title = L10n.text(title)
        self.detail = detail.map { L10n.text($0) }
    }

    init(_ title: String, detail: String?) {
        self.title = title
        self.detail = detail
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(MemeDockTypography.body)
            if let detail {
                Text(detail)
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct SettingsHeader: View {
    @Environment(\.colorScheme) private var scheme
    let key: L10nKey

    init(_ key: L10nKey) {
        self.key = key
    }

    var body: some View {
        Text(L10n.text(key))
            .font(MemeDockTypography.sectionLabel)
            .foregroundStyle(MemeDockPalette(scheme).secondary)
    }
}

struct SettingsFooter: View {
    @Environment(\.colorScheme) private var scheme
    let key: L10nKey

    init(_ key: L10nKey) {
        self.key = key
    }

    var body: some View {
        Text(L10n.text(key))
            .font(MemeDockTypography.caption)
            .foregroundStyle(MemeDockPalette(scheme).secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// A figure that rolls to its new value instead of being swapped, so a
/// refresh reads as the same number changing.
struct SettingsFigure: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let text: String
    let value: Double

    var body: some View {
        Text(text)
            .monospacedDigit()
            .foregroundStyle(MemeDockPalette(scheme).secondary)
            .contentTransition(reduceMotion ? .opacity : .numericText(value: value))
            .animation(reduceMotion ? nil : .snappy(duration: 0.3), value: value)
    }
}

/// A file or folder: its name and path on the left, the way to it on the right.
struct SettingsPathRow: View {
    @Environment(\.colorScheme) private var scheme
    let title: String
    let path: String

    var body: some View {
        LabeledContent {
            Button(L10n.text(.revealInFinder)) { revealInFinder(path) }
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(path)
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
                    .help(path)
            }
        }
    }
}

/// Work that has started and has not answered yet.
struct SettingsProgress: View {
    @Environment(\.colorScheme) private var scheme
    let key: L10nKey

    init(_ key: L10nKey = .applying) {
        self.key = key
    }

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.text(key))
                .foregroundStyle(MemeDockPalette(scheme).secondary)
        }
    }
}

@MainActor
func revealInFinder(_ path: String) {
    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
}
