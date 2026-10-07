use crate::i18n::{self, Key};
use gtk4::glib::object::IsA;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use memedock_core::{ArtifactLease, Library};
use memedock_domain::asset::ImageFormat;
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset};
use memedock_domain::identity::StickerId;
use memedock_domain::local::UsageAction;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

const CLIP_LOG: &str = "/tmp/memedock-clipboard.log";

pub fn log_process(event: &str) {
    clip_log(event);
}

fn clip_log(message: &str) {
    let line = format!("[memedock-clip pid={}] {message}", std::process::id());
    eprintln!("{line}");
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(CLIP_LOG)
    else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

pub struct Hold {
    clipboard: Option<Arc<ArtifactLease>>,
    shared: Option<Arc<ArtifactLease>>,
    busy: bool,
}

impl Hold {
    pub fn new() -> Self {
        Self {
            clipboard: None,
            shared: None,
            busy: false,
        }
    }

    pub fn release(&mut self) {
        self.clipboard.take();
        self.shared.take();
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
    pub source: ImageFormat,
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
    _anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    kind: CopyKind,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy {
        clip_log("复制被跳过，上一次还在进行");
        return false;
    }
    let mime = clipboard_mime(kind, request);
    clip_log(&format!(
        "复制开始 kind={} mime={mime}",
        copy_kind_name(kind)
    ));
    hold.borrow_mut().busy = true;
    glib::spawn_future_local(async move {
        let finish = fill_clipboard(&library, request, kind).await;
        hold.borrow_mut().busy = false;
        match &finish {
            Ok(_) => clip_log("复制流程结束：成功"),
            Err(message) => clip_log(&format!("复制流程结束：{message}")),
        }
        report(&notify, finish);
    });
    true
}

fn copy_kind_name(kind: CopyKind) -> &'static str {
    match kind {
        CopyKind::Image => "image",
        CopyKind::File => "file",
    }
}

fn clipboard_mime(kind: CopyKind, request: Request) -> &'static str {
    match kind {
        CopyKind::File => "text/uri-list",
        CopyKind::Image => {
            let (preset, _) = resolve(request.animated, request.preset, request.first_frame);
            match preset {
                ExportPreset::Original => request.source.mime(),
                ExportPreset::SmallJpeg => ImageFormat::Jpeg.mime(),
                ExportPreset::CompatiblePng | ExportPreset::WhiteBackground => {
                    ImageFormat::Png.mime()
                }
            }
        }
    }
}

pub fn spawn_save(
    library: Library,
    hold: Rc<RefCell<Hold>>,
    anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy {
        return false;
    }
    let Some(window) = parent_window(anchor) else {
        notify(Some(i18n::text(Key::FailureUnknown)));
        return false;
    };
    hold.borrow_mut().busy = true;
    glib::spawn_future_local(async move {
        let finish = save_resolved(&library, &window, request).await;
        hold.borrow_mut().busy = false;
        report(&notify, finish);
    });
    true
}

pub fn spawn_share(
    library: Library,
    hold: Rc<RefCell<Hold>>,
    anchor: &impl IsA<gtk4::Widget>,
    request: Request,
    notify: impl Fn(Option<&str>) + 'static,
) -> bool {
    if hold.borrow().busy {
        return false;
    }
    let Some(window) = parent_window(anchor) else {
        notify(Some(i18n::text(Key::FailureUnknown)));
        return false;
    };
    hold.borrow_mut().busy = true;
    glib::spawn_future_local(async move {
        let finish = launch_share(&library, &hold, &window, request).await;
        hold.borrow_mut().busy = false;
        report(&notify, finish);
    });
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
    request: Request,
    kind: CopyKind,
) -> Result<Settled, &'static str> {
    clip_log("开始导出");
    let exported = match kind {
        CopyKind::File => export_original(library, request.id).await,
        CopyKind::Image => {
            let (preset, policy) = resolve(request.animated, request.preset, request.first_frame);
            export_resolved(library, request.id, preset, policy).await
        }
    };
    let lease = match exported {
        Ok(lease) => lease,
        Err(message) => {
            clip_log(&format!("导出失败 {message}"));
            return Err(message);
        }
    };
    let metadata = lease.metadata().clone();
    clip_log(&format!(
        "导出完成 file={} mime={}",
        metadata.file_name, metadata.mime
    ));
    let payload = match kind {
        CopyKind::File => uri_list(&metadata.path),
        CopyKind::Image => match read_file(&metadata.path).await {
            Ok(bytes) => bytes,
            Err(message) => {
                clip_log(&format!("读取文件失败 {message}"));
                return Err(message);
            }
        },
    };
    clip_log(&format!("载荷 {} 字节", payload.len()));
    drop(lease);
    let mime = clipboard_mime(kind, request).to_owned();
    let handed = gio::spawn_blocking(move || give_to_desktop(&mime, &payload))
        .await
        .map_err(|_| i18n::text(Key::FailureUnknown))?;
    handed?;
    let action = match kind {
        CopyKind::Image => UsageAction::CopyImage,
        CopyKind::File => UsageAction::CopyFile,
    };
    record(library, request.id, action).await?;
    Ok(Settled::Toast(i18n::text(Key::ImageCopied)))
}

fn uri_list(path: &Path) -> Vec<u8> {
    format!("{}\r\n", gio::File::for_path(path).uri()).into_bytes()
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

async fn export_original(
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

async fn record(library: &Library, id: StickerId, action: UsageAction) -> Result<(), &'static str> {
    match library.record_use(id, action) {
        Ok(task) => task
            .wait()
            .await
            .map(|_| ())
            .map_err(|error| i18n::core(error.code())),
        Err(error) => Err(i18n::core(error.code())),
    }
}

async fn read_file(path: &Path) -> Result<Vec<u8>, &'static str> {
    let path = path.to_path_buf();
    gio::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(|_| i18n::text(Key::FailureUnknown))?
        .map_err(|_| i18n::text(Key::FailureIo))
}

fn give_to_desktop(mime: &str, bytes: &[u8]) -> Result<(), &'static str> {
    let mut child = std::process::Command::new("wl-copy")
        .arg("--type")
        .arg(mime)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| {
            clip_log(&format!("无法启动 wl-copy：{error}"));
            i18n::text(Key::FailureUnknown)
        })?;
    let Some(mut stdin) = child.stdin.take() else {
        clip_log("wl-copy 没有标准输入");
        return Err(i18n::text(Key::FailureUnknown));
    };
    if let Err(error) = stdin.write_all(bytes) {
        clip_log(&format!("写入 wl-copy 失败：{error}"));
        return Err(i18n::text(Key::FailureIo));
    }
    drop(stdin);
    match child.wait() {
        Ok(status) if status.success() => {
            clip_log(&format!(
                "桌面剪贴板已接收 mime={mime} bytes={}",
                bytes.len()
            ));
            Ok(())
        }
        Ok(status) => {
            clip_log(&format!("wl-copy 退出：{status}"));
            Err(i18n::text(Key::FailureUnknown))
        }
        Err(error) => {
            clip_log(&format!("等待 wl-copy 失败：{error}"));
            Err(i18n::text(Key::FailureUnknown))
        }
    }
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
