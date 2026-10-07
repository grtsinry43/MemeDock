use crate::i18n::{self, Key};
use gtk4::glib::object::IsA;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use memedock_core::{ArtifactLease, Library};
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset};
use memedock_domain::identity::StickerId;
use memedock_domain::local::UsageAction;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

pub struct Hold {
    pub clipboard: Rc<RefCell<crate::clipboard::State>>,
    shared: Option<Arc<ArtifactLease>>,
    busy: bool,
    closing: bool,
    task: Option<glib::JoinHandle<()>>,
}

impl Hold {
    pub fn new() -> Self {
        Self {
            clipboard: Rc::new(RefCell::new(crate::clipboard::State::default())),
            shared: None,
            busy: false,
            closing: false,
            task: None,
        }
    }

    pub fn release(&mut self) {
        self.clipboard.borrow_mut().release();
        self.shared.take();
    }

    pub fn begin_shutdown(&mut self) -> Option<glib::JoinHandle<()>> {
        self.closing = true;
        self.task.take()
    }
}

#[derive(Clone, Copy)]
pub enum CopyKind {
    Image,
    File,
}

#[derive(Clone, Copy)]
pub struct Request {
    pub id: StickerId,
    pub animated: bool,
    pub preset: ExportPreset,
    pub first_frame: bool,
}

pub fn resolve(
    animated: bool,
    preset: ExportPreset,
    first_frame: bool,
) -> (ExportPreset, AnimationPolicy) {
    if preset == ExportPreset::Original || (animated && !first_frame) {
        (ExportPreset::Original, AnimationPolicy::Preserve)
    } else if first_frame {
        (preset, AnimationPolicy::FirstFrame)
    } else {
        (preset, AnimationPolicy::Preserve)
    }
}

pub fn flattens(animated: bool, preset: ExportPreset) -> bool {
    animated && preset != ExportPreset::Original
}

pub fn preset_name(preset: ExportPreset) -> &'static str {
    match preset {
        ExportPreset::Original => i18n::text(Key::ExportOriginal),
        ExportPreset::CompatiblePng => i18n::text(Key::ExportPng),
        ExportPreset::WhiteBackground => i18n::text(Key::ExportWhite),
        ExportPreset::SmallJpeg => i18n::text(Key::ExportSmall),
    }
}

pub fn preset_hint(preset: ExportPreset) -> &'static str {
    match preset {
        ExportPreset::Original => i18n::text(Key::ExportOriginalHint),
        ExportPreset::CompatiblePng => i18n::text(Key::ExportPngHint),
        ExportPreset::WhiteBackground => i18n::text(Key::ExportWhiteHint),
        ExportPreset::SmallJpeg => i18n::text(Key::ExportSmallHint),
    }
}

pub fn spawn_copy(
    library: Library,
    hold: Rc<RefCell<Hold>>,
    anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    kind: CopyKind,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy || hold.borrow().closing {
        notify(Some(i18n::text(Key::FailureBusy)));
        return false;
    }
    let clipboard = anchor.upcast_ref::<gtk4::Widget>().clipboard();
    let state = Rc::clone(&hold.borrow().clipboard);
    crate::clipboard::watch(&state, &clipboard);
    hold.borrow_mut().busy = true;
    let tracked = Rc::clone(&hold);
    let task = glib::spawn_future_local(async move {
        let finish = fill_clipboard(&library, &state, &clipboard, request, kind).await;
        tracked.borrow_mut().busy = false;
        tracked.borrow_mut().task = None;
        report(&notify, finish);
    });
    hold.borrow_mut().task = Some(task);
    true
}

pub fn spawn_save(
    library: Library,
    hold: Rc<RefCell<Hold>>,
    anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy || hold.borrow().closing {
        notify(Some(i18n::text(Key::FailureBusy)));
        return false;
    }
    let Some(window) = parent_window(anchor) else {
        notify(Some(i18n::text(Key::FailureUnknown)));
        return false;
    };
    hold.borrow_mut().busy = true;
    let tracked = Rc::clone(&hold);
    let task = glib::spawn_future_local(async move {
        let finish = save_resolved(&library, &window, request).await;
        tracked.borrow_mut().busy = false;
        tracked.borrow_mut().task = None;
        report(&notify, finish);
    });
    hold.borrow_mut().task = Some(task);
    true
}

pub fn spawn_share(
    library: Library,
    hold: Rc<RefCell<Hold>>,
    anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy || hold.borrow().closing {
        notify(Some(i18n::text(Key::FailureBusy)));
        return false;
    }
    let Some(window) = parent_window(anchor) else {
        notify(Some(i18n::text(Key::FailureUnknown)));
        return false;
    };
    hold.borrow_mut().busy = true;
    let tracked = Rc::clone(&hold);
    let task = glib::spawn_future_local(async move {
        let finish = launch_share(&library, &tracked, &window, request).await;
        tracked.borrow_mut().busy = false;
        tracked.borrow_mut().task = None;
        report(&notify, finish);
    });
    hold.borrow_mut().task = Some(task);
    true
}

fn parent_window(anchor: &impl IsA<gtk4::Widget>) -> Option<gtk4::Window> {
    anchor
        .upcast_ref::<gtk4::Widget>()
        .root()
        .and_then(|root| root.downcast().ok())
}

enum Settled {
    Toast(&'static str),
    Silent,
}

fn report(notify: &impl Fn(Option<&str>), finish: Result<Settled, &'static str>) {
    match finish {
        Ok(Settled::Toast(message)) => notify(Some(message)),
        Ok(Settled::Silent) => notify(None),
        Err(message) => notify(Some(message)),
    }
}

async fn fill_clipboard(
    library: &Library,
    state: &Rc<RefCell<crate::clipboard::State>>,
    clipboard: &gtk4::gdk::Clipboard,
    request: Request,
    kind: CopyKind,
) -> Result<Settled, &'static str> {
    let lease = match kind {
        CopyKind::File => export_original(library, request.id).await,
        CopyKind::Image => {
            let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
            export_resolved(library, request.id, preset, policy).await
        }
    }?;
    let representation = match kind {
        CopyKind::File => crate::clipboard::Representation::File,
        CopyKind::Image => crate::clipboard::Representation::Image,
    };
    let prepared = crate::clipboard::prepare(lease, representation).await?;
    crate::clipboard::install(state, clipboard, library, prepared).await?;
    let action = match kind {
        CopyKind::Image => UsageAction::CopyImage,
        CopyKind::File => UsageAction::CopyFile,
    };
    record(library, request.id, action).await?;
    Ok(Settled::Toast(i18n::text(Key::ImageCopied)))
}

async fn save_resolved(
    library: &Library,
    window: &gtk4::Window,
    request: Request,
) -> Result<Settled, &'static str> {
    let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
    let lease = export_resolved(library, request.id, preset, policy).await?;
    let metadata = lease.metadata().clone();
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(i18n::text(Key::SaveAs));
    dialog.set_initial_name(Some(&metadata.file_name));
    let chosen = match dialog.save_future(Some(window)).await {
        Ok(file) => file,
        Err(_) => return Ok(Settled::Silent),
    };
    let Some(destination) = chosen.path() else {
        return Err(i18n::text(Key::FailureIo));
    };
    let source = metadata.path.clone();
    let copied = gio::spawn_blocking(move || std::fs::copy(source, destination))
        .await
        .map_err(|_| i18n::text(Key::FailureUnknown))?;
    copied.map_err(|_| i18n::text(Key::FailureIo))?;
    record(library, request.id, UsageAction::ExportSaved).await?;
    Ok(Settled::Toast(i18n::text(Key::Saved)))
}

async fn launch_share(
    library: &Library,
    hold: &Rc<RefCell<Hold>>,
    window: &gtk4::Window,
    request: Request,
) -> Result<Settled, &'static str> {
    let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
    let lease = export_resolved(library, request.id, preset, policy).await?;
    let path = lease.metadata().path.clone();
    let launcher = gtk4::FileLauncher::new(Some(&gio::File::for_path(path)));
    launcher.set_always_ask(true);
    match launcher.launch_future(Some(window)).await {
        Ok(()) => {
            hold.borrow_mut().shared = Some(lease);
            record(library, request.id, UsageAction::ShareLaunched).await?;
            Ok(Settled::Silent)
        }
        Err(error)
            if error.matches(gtk4::DialogError::Dismissed)
                || error.matches(gtk4::DialogError::Cancelled) =>
        {
            Ok(Settled::Silent)
        }
        Err(_) => Err(i18n::text(Key::FailureUnknown)),
    }
}

pub(crate) async fn export_original(
    library: &Library,
    id: StickerId,
) -> Result<Arc<ArtifactLease>, &'static str> {
    match library.export_original(id) {
        Ok(task) => task.wait().await.map_err(|error| i18n::core(error.code())),
        Err(error) => Err(i18n::core(error.code())),
    }
}

async fn export_resolved(
    library: &Library,
    id: StickerId,
    preset: ExportPreset,
    policy: AnimationPolicy,
) -> Result<Arc<ArtifactLease>, &'static str> {
    if preset == ExportPreset::Original {
        return export_original(library, id).await;
    }
    let options = ExportOptions::for_preset(preset, policy);
    match library.export(id, options) {
        Ok(task) => task.wait().await.map_err(|error| i18n::core(error.code())),
        Err(error) => Err(i18n::core(error.code())),
    }
}

pub(crate) async fn record(
    library: &Library,
    id: StickerId,
    action: UsageAction,
) -> Result<(), &'static str> {
    match library.record_use(id, action) {
        Ok(task) => task
            .wait()
            .await
            .map(|_| ())
            .map_err(|error| i18n::core(error.code())),
        Err(error) => Err(i18n::core(error.code())),
    }
}

pub(crate) async fn read_file(path: &Path) -> Result<Vec<u8>, &'static str> {
    let path = path.to_path_buf();
    gio::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(|_| i18n::text(Key::FailureUnknown))?
        .map_err(|_| i18n::text(Key::FailureIo))
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use memedock_domain::export::{AnimationPolicy, ExportPreset};

    #[test]
    fn animated_main_action_stays_original_until_first_frame_is_requested() {
        assert_eq!(
            resolve(false, ExportPreset::CompatiblePng, false),
            (ExportPreset::CompatiblePng, AnimationPolicy::Preserve)
        );
        assert_eq!(
            resolve(true, ExportPreset::Original, false),
            (ExportPreset::Original, AnimationPolicy::Preserve)
        );
        assert_eq!(
            resolve(true, ExportPreset::SmallJpeg, false),
            (ExportPreset::Original, AnimationPolicy::Preserve)
        );
        assert_eq!(
            resolve(true, ExportPreset::WhiteBackground, true),
            (ExportPreset::WhiteBackground, AnimationPolicy::FirstFrame)
        );
    }
}
