//! Window level keyboard accelerators.
//!
//! The table lives here so a binding can never drift from the hint the UI shows
//! for it, and so the question "when must a shortcut step aside for a focused
//! text widget" is answered once. Wiring stays with the module that owns the
//! capability: the library view registers selection, search and delete, the
//! import controls register import.

use crate::i18n::{self, Key};
use gtk4::{glib, prelude::*};
use std::rc::Rc;

/// Keys the window reacts to itself. Everything else belongs to the focused
/// widget: the grid's own arrows and activation, text editing, and the import
/// overlay's paste chord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Copy the image of the sticker under focus.
    CopyImage,
    /// Select every sticker the grid has loaded.
    SelectAll,
    /// Drop the current selection.
    UnselectAll,
    /// Move keyboard focus to the search field.
    FocusSearch,
    /// Open the import picker.
    Import,
    /// Delete the selected stickers.
    DeleteSelection,
}

/// Every action, in the order the window mentions them. `install` walks this
/// list, so a new variant is a compile error at the call site until it is
/// either wired or explicitly left unbound.
pub const ALL: [Action; 6] = [
    Action::CopyImage,
    Action::SelectAll,
    Action::UnselectAll,
    Action::FocusSearch,
    Action::Import,
    Action::DeleteSelection,
];

impl Action {
    /// The trigger as `ShortcutTrigger::parse_string` reads it.
    pub const fn trigger(self) -> &'static str {
        match self {
            Action::CopyImage => "<Control>c",
            Action::SelectAll => "<Control>a",
            Action::UnselectAll => "<Control><Shift>a",
            Action::FocusSearch => "<Control>f",
            Action::Import => "<Control>i",
            Action::DeleteSelection => "Delete",
        }
    }

    /// The same key written for people, for tooltips.
    pub const fn label(self) -> &'static str {
        match self {
            Action::CopyImage => "Ctrl+C",
            Action::SelectAll => "Ctrl+A",
            Action::UnselectAll => "Ctrl+Shift+A",
            Action::FocusSearch => "Ctrl+F",
            Action::Import => "Ctrl+I",
            Action::DeleteSelection => "Delete",
        }
    }

    /// Whether a focused text widget owns this key. Ctrl+A selects its content,
    /// Ctrl+C copies it and Delete edits it, so the window must not take those
    /// over while the caret is active. Ctrl+F and Ctrl+I mean the same thing
    /// wherever the caret happens to be.
    pub const fn yields_to_text(self) -> bool {
        !matches!(self, Action::FocusSearch | Action::Import)
    }
}

/// What holds keyboard focus, as far as shortcuts are concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Text,
    Other,
}

/// Whether `action` runs instead of leaving the key to the focused widget.
pub fn handles(action: Action, focus: Focus) -> bool {
    !(action.yields_to_text() && focus == Focus::Text)
}

/// Reads the focus out of the widget tree the way GTK reports it.
pub fn focus_of(widget: &impl IsA<gtk4::Widget>) -> Focus {
    let editing = widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| {
            focus.is::<gtk4::Text>()
                || focus.is::<gtk4::TextView>()
                || focus.is::<gtk4::Entry>()
                || focus.is::<gtk4::SearchEntry>()
        });
    if editing { Focus::Text } else { Focus::Other }
}

pub type Handler = Rc<dyn Fn()>;

/// A label that names the key next to the action it triggers.
pub fn hint(key: Key, action: Action) -> String {
    format!("{} ({})", i18n::text(key), action.label())
}

/// Registers the accelerators on `host`. The handler decides what each action
/// does, and returning `None` leaves that key unbound. A handled key stops here,
/// so the focused widget never runs the action a second time through its own
/// bindings.
pub fn install(host: &impl IsA<gtk4::Widget>, handler: impl Fn(Action) -> Option<Handler>) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Global);
    for action in ALL {
        let Some(run) = handler(action) else {
            continue;
        };
        let Some(trigger) = gtk4::ShortcutTrigger::parse_string(action.trigger()) else {
            continue;
        };
        let callback = gtk4::CallbackAction::new(move |widget, _| {
            if !handles(action, focus_of(widget)) {
                return glib::Propagation::Proceed;
            }
            run();
            glib::Propagation::Stop
        });
        controller.add_shortcut(gtk4::Shortcut::new(Some(trigger), Some(callback)));
    }
    host.add_controller(controller);
}

#[cfg(test)]
mod tests {
    use super::{ALL, Action, Focus, handles};

    /// The modifier groups GTK's accelerator parser accepts, in the angle-bracket
    /// form `ShortcutTrigger::parse_string` expects.
    const MODIFIERS: [&str; 8] = [
        "<Control>",
        "<Ctrl>",
        "<Shift>",
        "<Alt>",
        "<Meta>",
        "<Super>",
        "<Hyper>",
        "<Primary>",
    ];

    /// Whether `trigger` has the shape GTK's accelerator parser reads. Asking GTK
    /// itself would need `gtk::init()`, which a headless test binary has no display
    /// for, so the grammar is restated here and the real parse is left to the
    /// manual pass.
    fn well_formed(trigger: &str) -> bool {
        let mut rest = trigger;
        while let Some(without_open) = rest.strip_prefix('<') {
            let Some(end) = without_open.find('>') else {
                return false;
            };
            if !MODIFIERS.contains(&&rest[..end + 2]) {
                return false;
            }
            rest = &without_open[end + 1..];
        }
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-_ ".contains(c))
    }

    #[test]
    fn every_action_has_a_well_formed_trigger_and_a_label() {
        for action in ALL {
            assert!(
                well_formed(action.trigger()),
                "{action:?} has a trigger GTK cannot parse"
            );
            assert!(!action.label().is_empty(), "{action:?} has no label");
        }
    }

    #[test]
    fn bindings_are_the_documented_ones() {
        assert_eq!(Action::SelectAll.trigger(), "<Control>a");
        assert_eq!(Action::UnselectAll.trigger(), "<Control><Shift>a");
        assert_eq!(Action::FocusSearch.trigger(), "<Control>f");
        assert_eq!(Action::Import.trigger(), "<Control>i");
        assert_eq!(Action::CopyImage.trigger(), "<Control>c");
        assert_eq!(Action::DeleteSelection.trigger(), "Delete");
    }

    #[test]
    fn the_shape_check_rejects_what_gtk_would_not_read() {
        assert!(!well_formed(""));
        assert!(!well_formed("<Control"));
        assert!(!well_formed("<control>a"));
        assert!(!well_formed("<NotAModifier>a"));
        assert!(well_formed("Delete"));
        assert!(well_formed("<Control><Shift>a"));
    }

    #[test]
    fn no_two_actions_share_a_key() {
        for (index, action) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
                assert_ne!(action.trigger(), other.trigger());
                assert_ne!(action.label(), other.label());
            }
        }
    }

    #[test]
    fn text_editing_keys_stay_with_the_text_widget() {
        for action in [
            Action::CopyImage,
            Action::SelectAll,
            Action::UnselectAll,
            Action::DeleteSelection,
        ] {
            assert!(!handles(action, Focus::Text), "{action:?} steals a key");
            assert!(handles(action, Focus::Other));
        }
    }

    #[test]
    fn window_wide_keys_work_while_typing() {
        for action in [Action::FocusSearch, Action::Import] {
            assert!(handles(action, Focus::Text));
            assert!(handles(action, Focus::Other));
        }
    }
}
