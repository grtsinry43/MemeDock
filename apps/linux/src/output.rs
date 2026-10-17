use crate::i18n::{self, Key};
use gtk4::glib::object::IsA;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use memedock_core::{ArtifactLease, Library};
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset};
use memedock_domain::identity::StickerId;
use memedock_domain::local::UsageAction;
use std::cell::Cell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

pub struct Selection {
    pub preferences: Rc<RefCell<crate::preferences::Preferences>>,
    pub temporary: Cell<Option<ExportPreset>>,
}

impl Selection {
    pub fn effective_override(&self, animated: bool) -> Option<ExportPreset> {
        self.temporary
            .get()
            .filter(|preset| presets(animated).contains(preset))
    }
    pub fn get(&self, action: crate::preferences::OutputAction, animated: bool) -> ExportPreset {
        self.effective_override(animated)
            .unwrap_or_else(|| self.preferences.borrow().output.preset(action, animated))
    }
}

pub struct Hold {
    pub clipboard: Rc<RefCell<crate::clipboard::State>>,
    shared: Option<Arc<ArtifactLease>>,
    busy: bool,
    closing: bool,
    task: Option<glib::JoinHandle<()>>,
    export: Option<memedock_core::tasks::TaskController>,
}

impl Hold {
    pub fn is_busy(&self) -> bool {
        self.busy || self.closing
    }
    pub fn new() -> Self {
        Self {
            clipboard: Rc::new(RefCell::new(crate::clipboard::State::default())),
            shared: None,
            busy: false,
            closing: false,
            task: None,
            export: None,
        }
    }

    pub fn release(&mut self) {
        self.clipboard.borrow_mut().release();
        self.shared.take();
    }

    pub fn cancel_output(&self) {
        if let Some(export) = &self.export {
            export.cancel();
        }
    }

    pub fn begin_shutdown(&mut self) -> Option<glib::JoinHandle<()>> {
        self.closing = true;
        self.cancel_output();
        self.task.take()
    }
}

#[derive(Clone, Copy)]
pub enum CopyKind {
    Image,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    if first_frame {
        (ExportPreset::CompatiblePng, AnimationPolicy::FirstFrame)
    } else if matches!(
        preset,
        ExportPreset::BestCompatibility | ExportPreset::CompatibleGif
    ) {
        (preset, AnimationPolicy::Preserve)
    } else if preset == ExportPreset::Original || animated {
        (ExportPreset::Original, AnimationPolicy::Preserve)
    } else {
        (preset, AnimationPolicy::Preserve)
    }
}

pub fn presets(animated: bool) -> &'static [ExportPreset] {
    if animated {
        &[ExportPreset::BestCompatibility, ExportPreset::Original]
    } else {
        &[
            ExportPreset::BestCompatibility,
            ExportPreset::Original,
            ExportPreset::WhiteBackground,
            ExportPreset::SmallJpeg,
        ]
    }
}

pub fn preset_name(preset: ExportPreset) -> &'static str {
    match preset {
        ExportPreset::Original => i18n::text(Key::ExportOriginal),
        ExportPreset::CompatiblePng => i18n::text(Key::ExportPng),
        ExportPreset::WhiteBackground => i18n::text(Key::ExportWhite),
        ExportPreset::SmallJpeg => i18n::text(Key::ExportSmall),
        ExportPreset::CompatibleGif => i18n::text(Key::ExportGif),
        ExportPreset::BestCompatibility => i18n::text(Key::ExportBest),
    }
}

pub fn preset_hint(preset: ExportPreset) -> &'static str {
    match preset {
        ExportPreset::Original => i18n::text(Key::ExportOriginalHint),
        ExportPreset::CompatiblePng => i18n::text(Key::ExportPngHint),
        ExportPreset::WhiteBackground => i18n::text(Key::ExportWhiteHint),
        ExportPreset::SmallJpeg => i18n::text(Key::ExportSmallHint),
        ExportPreset::CompatibleGif => i18n::text(Key::ExportGifHint),
        ExportPreset::BestCompatibility => i18n::text(Key::ExportBestHint),
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
        let finish = fill_clipboard(&library, &tracked, &state, &clipboard, request, kind).await;
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
        let finish = save_resolved(&library, &tracked, &window, request).await;
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
        Err(message) if message == i18n::core(memedock_core::ErrorCode::Cancelled) => notify(None),
        Err(message) => notify(Some(message)),
    }
}

async fn fill_clipboard(
    library: &Library,
    hold: &Rc<RefCell<Hold>>,
    state: &Rc<RefCell<crate::clipboard::State>>,
    clipboard: &gtk4::gdk::Clipboard,
    request: Request,
    kind: CopyKind,
) -> Result<Settled, &'static str> {
    let lease = match kind {
        CopyKind::File => {
            export_resolved(
                library,
                request.id,
                ExportPreset::Original,
                AnimationPolicy::Preserve,
                Some(hold),
            )
            .await
        }
        CopyKind::Image => {
            let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
            export_resolved(library, request.id, preset, policy, Some(hold)).await
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
    hold: &Rc<RefCell<Hold>>,
    window: &gtk4::Window,
    request: Request,
) -> Result<Settled, &'static str> {
    let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
    let lease = export_resolved(library, request.id, preset, policy, Some(hold)).await?;
    let metadata = lease.metadata().clone();
    let dialog = gtk4::FileDialog::new();
    i18n::bind(&dialog, "title", Key::SaveAs);
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
    let lease = export_resolved(library, request.id, preset, policy, Some(hold)).await?;
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

pub(crate) async fn export_resolved(
    library: &Library,
    id: StickerId,
    preset: ExportPreset,
    policy: AnimationPolicy,
    hold: Option<&Rc<RefCell<Hold>>>,
) -> Result<Arc<ArtifactLease>, &'static str> {
    let options = ExportOptions::for_preset(preset, policy);
    let task = library
        .export(id, options)
        .map_err(|error| i18n::core(error.code()))?;
    if let Some(hold) = hold {
        hold.borrow_mut().export = Some(task.controller());
    }
    let result = task.wait().await.map_err(|error| i18n::core(error.code()));
    if let Some(hold) = hold {
        hold.borrow_mut().export = None;
    }
    result
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
pub(crate) mod tests {
    use super::resolve;
    use memedock_domain::export::{AnimationPolicy, ExportPreset};

    pub(crate) async fn verify_platform_output(
        library: &memedock_core::Library,
        widget: &gtk4::Widget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use gtk4::{gio, glib, prelude::*};
        use std::{cell::RefCell, rc::Rc};
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
            for color in [image::Rgba([255, 0, 0, 255]), image::Rgba([0, 0, 255, 255])] {
                encoder.encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(12, 8, color),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(500, 1),
                ))?;
            }
        }
        let input = library.create_import_input()?.wait().await?;
        std::fs::write(input.path(), &bytes)?;
        let imported = library
            .import_staged(
                input,
                memedock_core::ImportOptions {
                    original_name: "animated.gif".into(),
                    ..Default::default()
                },
            )?
            .wait()
            .await?;
        let request = super::Request {
            id: imported.sticker.id(),
            animated: true,
            preset: ExportPreset::BestCompatibility,
            first_frame: false,
        };
        let hold = Rc::new(RefCell::new(super::Hold::new()));
        let finish = Rc::new(RefCell::new(None));
        let settled = Rc::clone(&finish);
        assert!(super::spawn_copy(
            library.clone(),
            Rc::clone(&hold),
            widget,
            request,
            super::CopyKind::Image,
            move |error| {
                *settled.borrow_mut() = Some(error.map(str::to_owned));
            }
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while hold.borrow().is_busy() {
            assert!(
                std::time::Instant::now() < deadline,
                "clipboard export did not settle"
            );
            glib::timeout_future(std::time::Duration::from_millis(10)).await;
        }
        // Success is reported with the normal copy toast.
        assert_eq!(
            finish.borrow().as_ref(),
            Some(&Some(
                crate::i18n::text(crate::i18n::Key::ImageCopied).to_owned()
            ))
        );
        let formats = widget.clipboard().formats();
        assert!(
            formats.contain_mime_type("image/gif"),
            "formats: {:?}; provider: {:?}",
            formats.mime_types(),
            widget
                .clipboard()
                .content()
                .map(|provider| provider.formats().mime_types())
        );
        assert!(!formats.contain_mime_type("image/png"));
        // GDK adds deserializable GTypes to Clipboard::formats itself. Inspect
        // our provider to ensure we never explicitly offer a static texture.
        let content = widget
            .clipboard()
            .content()
            .ok_or("local clipboard provider missing")?;
        assert!(
            !content
                .formats()
                .contains_type(gtk4::gdk::Texture::static_type())
        );

        let lease = super::export_resolved(
            library,
            request.id,
            request.preset,
            AnimationPolicy::Preserve,
            None,
        )
        .await
        .map_err(std::io::Error::other)?;
        let prepared = crate::clipboard::prepare(lease, crate::clipboard::Representation::Drag)
            .await
            .map_err(std::io::Error::other)?;
        let stream = gio::MemoryOutputStream::new_resizable();
        prepared
            .provider
            .write_mime_type_future("image/gif", &stream, glib::Priority::DEFAULT)
            .await?;
        stream.close_future(glib::Priority::DEFAULT).await?;
        assert_eq!(
            stream.steal_as_bytes().as_ref(),
            std::fs::read(&prepared.lease.metadata().path)?
        );
        assert!(
            prepared
                .provider
                .formats()
                .contains_type(gtk4::gdk::FileList::static_type())
        );
        crate::drag::verify_preparation(library, request).await?;
        hold.borrow_mut().release();
        Ok(())
    }

    #[test]
    fn legacy_static_animation_requests_keep_original_until_first_frame_is_requested() {
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
            (ExportPreset::CompatiblePng, AnimationPolicy::FirstFrame)
        );
    }

    #[test]
    fn adaptive_animation_reaches_core_and_first_frame_is_always_png() {
        for animated in [false, true] {
            assert_eq!(
                resolve(animated, ExportPreset::BestCompatibility, false),
                (ExportPreset::BestCompatibility, AnimationPolicy::Preserve)
            );
            for &preset in super::presets(animated) {
                assert_eq!(
                    resolve(animated, preset, true),
                    (ExportPreset::CompatiblePng, AnimationPolicy::FirstFrame)
                );
            }
        }
    }
}
