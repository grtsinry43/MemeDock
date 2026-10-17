import AppKit
import SwiftUI

/// Which build this is, which library and device it speaks for, and the
/// licenses of what ships inside it.
struct AboutSettings: View {
    @Environment(\.colorScheme) private var scheme
    @ObservedObject var session: LibrarySession
    let status: StatusCenter

    var body: some View {
        SettingsForm {
            Section { identity }
            Section {
                idRow(.libraryId, session.identity?.libraryId)
                idRow(.deviceId, session.identity?.deviceId)
            }
            Section {
                LabeledContent(L10n.text(.app)) {
                    Text(L10n.text(.mitLicense))
                        .foregroundStyle(MemeDockPalette(scheme).secondary)
                }
                LabeledContent {
                    HStack(spacing: 12) {
                        Text(L10n.text(.openFontLicense))
                            .foregroundStyle(MemeDockPalette(scheme).secondary)
                        Button(L10n.text(.view)) { openFontLicense() }
                            .disabled(Self.fontLicense == nil)
                    }
                } label: {
                    Text(L10n.text(.quicksandFont))
                }
            } header: {
                SettingsHeader(.licenses)
            }
        }
    }

    private var identity: some View {
        HStack(spacing: 14) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 56, height: 56)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(L10n.text(.app))
                    .font(MemeDockTypography.sectionTitle)
                Text(L10n.text(.version, Self.version ?? "—"))
                    .font(MemeDockTypography.caption)
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .textSelection(.enabled)
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
    }

    private func idRow(_ title: L10nKey, _ value: String?) -> some View {
        LabeledContent {
            HStack(spacing: 8) {
                Text(value ?? "—")
                    .font(MemeDockTypography.caption)
                    .monospaced()
                    .foregroundStyle(MemeDockPalette(scheme).secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
                Button { copy(value) } label: {
                    Image(systemName: "doc.on.doc")
                }
                .buttonStyle(.borderless)
                .help(L10n.text(.copy))
                .accessibilityLabel(L10n.text(.copy))
                .disabled(value == nil)
            }
        } label: {
            Text(L10n.text(title))
        }
    }

    private func copy(_ value: String?) {
        guard let value else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(value, forType: .string)
        status.show(.idCopied)
    }

    private func openFontLicense() {
        guard let url = Self.fontLicense else { return }
        NSWorkspace.shared.open(url)
    }

    /// Only a bundled build carries a version; `swift run` has no Info.plist.
    private static var version: String? {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
    }

    /// Copied into the bundle next to the font by tools/macos/build.sh.
    private static var fontLicense: URL? {
        Bundle.main.url(forResource: "Quicksand-OFL", withExtension: "txt")
    }
}
