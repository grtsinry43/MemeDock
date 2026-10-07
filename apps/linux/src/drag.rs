use crate::clipboard::{self, Prepared};
use crate::i18n::{self, Key};
use gtk4::prelude::*;
use gtk4::{gdk, glib};
use memedock_core::Library;
use memedock_domain::identity::StickerId;
use std::cell::RefCell;
use std::rc::Rc;

/// One preparation and one ready payload per window, not one export per grid item.
#[derive(Default)]
pub struct State {
    target: Option<StickerId>,
    ready: Option<Prepared>,
    failure: Option<&'static str>,
    pending: Option<Prepared>,
    task: Option<glib::JoinHandle<()>>,
    active: Option<(gdk::Drag, Prepared)>,
    closing: bool,
}

pub fn cancel(state: &Rc<RefCell<State>>, id: Option<StickerId>) {
    let mut state = state.borrow_mut();
    if id.is_some() && state.target != id {
        return;
    }
    state.target = None;
    state.ready.take();
    state.failure = None;
    // Let an admitted decode settle before preparing the next target. Aborting a
    // blocking decoder's await does not stop that decoder and would defeat the bound.
}

pub fn shutdown(state: &Rc<RefCell<State>>) {
    state.borrow_mut().closing = true;
    cancel(state, None);
    let task = state.borrow_mut().task.take();
    if let Some(task) = task {
        task.abort();
    }
    state.borrow_mut().active.take();
    state.borrow_mut().pending.take();
}

fn prepare(
    state: &Rc<RefCell<State>>,
    id: StickerId,
    library: &Library,
    notify: &Rc<dyn Fn(&str)>,
) {
    if state.borrow().closing
        || (state.borrow().target == Some(id)
            && (state.borrow().ready.is_some()
                || state.borrow().task.is_some()
                || state.borrow().failure.is_some()))
    {
        return;
    }
    cancel(state, None);
    state.borrow_mut().target = Some(id);
    if state.borrow().task.is_some() {
        return;
    }
    let tracked = Rc::clone(state);
    let library = library.clone();
    let notify = Rc::clone(notify);
    let task = glib::spawn_future_local(async move {
        let result = match crate::output::export_original(&library, id).await {
            Ok(lease) => clipboard::prepare(lease, clipboard::Representation::Drag).await,
            Err(error) => Err(error),
        };
        let mut state = tracked.borrow_mut();
        state.task = None;
        if state.closing {
            return;
        }
        if state.target != Some(id) {
            let next = state.target;
            drop(state);
            if let Some(next) = next {
                prepare(&tracked, next, &library, &notify);
            }
            return;
        }
        match result {
            Ok(prepared) => state.ready = Some(prepared),
            Err(message) => {
                state.failure = Some(message);
                drop(state);
                notify(message);
            }
        }
    });
    state.borrow_mut().task = Some(task);
}

pub fn bind(
    widget: &impl IsA<gtk4::Widget>,
    state: Rc<RefCell<State>>,
    library: Library,
    target: Rc<dyn Fn() -> Option<StickerId>>,
    notify: Rc<dyn Fn(&str)>,
) {
    let motion = gtk4::EventControllerMotion::new();
    let tracked = Rc::clone(&state);
    let source = library.clone();
    let item = Rc::clone(&target);
    let message = Rc::clone(&notify);
    motion.connect_enter(move |_, _, _| {
        if let Some(id) = item() {
            prepare(&tracked, id, &source, &message);
        }
    });
    // Keep the single cached payload across pointer/focus leave. GTK can emit
    // these while recognizing a drag; invalidating here makes each retry cold.
    widget.add_controller(motion);

    let focus = gtk4::EventControllerFocus::new();
    let tracked = Rc::clone(&state);
    let source = library.clone();
    let item = Rc::clone(&target);
    let message = Rc::clone(&notify);
    focus.connect_enter(move |_| {
        if let Some(id) = item() {
            prepare(&tracked, id, &source, &message);
        }
    });
    widget.add_controller(focus);

    let click = gtk4::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let tracked = Rc::clone(&state);
    let source = library.clone();
    let item = Rc::clone(&target);
    let message = Rc::clone(&notify);
    click.connect_pressed(move |_, _, _, _| {
        if let Some(id) = item() {
            // A new deliberate press may retry a transient export failure.
            if tracked.borrow().target == Some(id) {
                tracked.borrow_mut().failure = None;
            }
            prepare(&tracked, id, &source, &message);
        }
    });
    widget.add_controller(click);

    let drag = gtk4::DragSource::new();
    drag.set_actions(gdk::DragAction::COPY);
    let tracked = Rc::clone(&state);
    let item = Rc::clone(&target);
    drag.connect_prepare(move |_, _, _| {
        let id = item()?;
        let provider = {
            let mut state = tracked.borrow_mut();
            state.pending = if state.target == Some(id) && !state.closing {
                state.ready.as_ref().cloned()
            } else {
                None
            };
            // The prepared signal's exact payload must survive rebinding or a
            // pointer leave before drag-begin; never borrow the next tile's lease.
            state
                .pending
                .as_ref()
                .map(|pending| pending.provider.clone())
        };
        if provider.is_none() {
            prepare(&tracked, id, &library, &notify);
            let failure = tracked.borrow().failure;
            notify(failure.unwrap_or_else(|| i18n::text(Key::SharePreparing)));
        }
        provider
    });
    let tracked = Rc::clone(&state);
    drag.connect_drag_begin(move |_, drag| {
        let mut state = tracked.borrow_mut();
        if let Some(prepared) = state.pending.take() {
            state.active = Some((drag.clone(), prepared));
        }
    });
    let tracked = Rc::clone(&state);
    drag.connect_drag_cancel(move |_, drag, _| {
        let mut state = tracked.borrow_mut();
        if state
            .active
            .as_ref()
            .is_none_or(|(current, _)| current == drag)
        {
            state.active.take();
            state.pending.take();
        }
        false
    });
    drag.connect_drag_end(move |_, drag, _| {
        let mut state = state.borrow_mut();
        if state
            .active
            .as_ref()
            .is_none_or(|(current, _)| current == drag)
        {
            state.active.take();
            state.pending.take();
        }
    });
    widget.add_controller(drag);
}
