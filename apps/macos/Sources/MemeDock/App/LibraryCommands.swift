import SwiftUI

private struct LibrarySearchFocusKey: FocusedValueKey {
    typealias Value = () -> Void
}

/// What the focused library window offers the File menu: the two ways in and
/// the one way out.
struct LibraryFileActions {
    let available: Bool
    let files: () -> Void
    let telegram: () -> Void
    let canDelete: Bool
    let delete: () -> Void
}

private struct LibraryFileKey: FocusedValueKey {
    typealias Value = LibraryFileActions
}

extension FocusedValues {
    var librarySearchFocus: (() -> Void)? {
        get { self[LibrarySearchFocusKey.self] }
        set { self[LibrarySearchFocusKey.self] = newValue }
    }

    var libraryFile: LibraryFileActions? {
        get { self[LibraryFileKey.self] }
        set { self[LibraryFileKey.self] = newValue }
    }
}

struct LibraryCommands: Commands {
    @FocusedValue(\.librarySearchFocus) private var focusSearch
    @FocusedValue(\.libraryFile) private var fileActions

    var body: some Commands {
        // The library has nothing to create from scratch; its File menu is the
        // two ways stickers come in, the same two the toolbar's + offers, and
        // the way they leave for the trash.
        CommandGroup(replacing: .newItem) {
            Button(L10n.text(.importFromFiles)) { fileActions?.files() }
                .keyboardShortcut("o")
                .disabled(fileActions?.available != true)
            Button(L10n.text(.importFromTelegramPack)) { fileActions?.telegram() }
                .keyboardShortcut("o", modifiers: [.command, .shift])
                .disabled(fileActions?.available != true)
            Divider()
            // No ⌘⌫ here: the responder handles it, so text fields keep
            // their own ⌘⌫ (delete to line start) while editing.
            Button(L10n.text(.deleteStickerTitle)) { fileActions?.delete() }
                .disabled(fileActions?.canDelete != true)
        }
        CommandGroup(after: .textEditing) {
            Button(L10n.text(.search)) { focusSearch?() }
                .keyboardShortcut("f")
                .disabled(focusSearch == nil)
        }
        // Grid density is a display choice, so it lives in the View menu.
        // Command-equals rather than command-plus: the plus sign needs shift on
        // a US layout and would never match.
        CommandGroup(after: .toolbar) {
            Button(L10n.text(.largerStickers)) { Preferences.shared.largerStickers() }
                .keyboardShortcut("=", modifiers: .command)
            Button(L10n.text(.smallerStickers)) { Preferences.shared.smallerStickers() }
                .keyboardShortcut("-", modifiers: .command)
            Button(L10n.text(.defaultStickerSize)) { Preferences.shared.resetStickerSize() }
                .keyboardShortcut("0", modifiers: .command)
        }
    }
}
