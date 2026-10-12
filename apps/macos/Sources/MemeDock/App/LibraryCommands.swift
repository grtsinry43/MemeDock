import SwiftUI

private struct LibrarySearchFocusKey: FocusedValueKey {
    typealias Value = () -> Void
}

extension FocusedValues {
    var librarySearchFocus: (() -> Void)? {
        get { self[LibrarySearchFocusKey.self] }
        set { self[LibrarySearchFocusKey.self] = newValue }
    }
}

struct LibraryCommands: Commands {
    @FocusedValue(\.librarySearchFocus) private var focusSearch

    var body: some Commands {
        CommandGroup(after: .textEditing) {
            Button(L10n.text(.search)) { focusSearch?() }
                .keyboardShortcut("f")
                .disabled(focusSearch == nil)
        }
    }
}
