use super::*;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use memedock_core::{ImportOptions, LibraryConfig};
use std::{
    error::Error,
    io,
    path::Path,
    process::{Child, Command as Process, Stdio},
    time::Instant,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn automatic_sync_requires_credentials_an_active_binding_and_an_unpaused_session() {
    let mut state = State::default();
    assert!(!state.automatic());
    state.loaded = true;
    state.credential = true;
    state.status.connected = true;
    assert!(state.automatic());
    state.status.paused = true;
    assert!(!state.automatic());
    state.status.paused = false;
    state.suspended = true;
    assert!(!state.automatic());
    state.suspended = false;
    state.terminal = true;
    assert!(!state.automatic());
    state.status.error = Some("unauthorized".into());
    assert!(state.needs_authorization());
}

pub(crate) async fn wait_until(test: impl Fn() -> bool) -> TestResult {
    glib::future_with_timeout(Duration::from_secs(60), async {
        while !test() {
            glib::timeout_future(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|_| io::Error::other("platform state did not settle"))?;
    Ok(())
}
pub(crate) fn named(parent: &impl IsA<gtk4::Widget>, name: &str) -> Option<gtk4::Widget> {
    let widget = parent.upcast_ref::<gtk4::Widget>();
    if widget.widget_name() == name {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = named(&current, name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
pub(crate) async fn import(
    library: &Library,
    name: &str,
    color: u8,
) -> TestResult<memedock_domain::sticker::Sticker> {
    let input = library.create_import_input()?.wait().await?;
    image::RgbaImage::from_pixel(3, 2, image::Rgba([color, 43, 91, 255]))
        .save_with_format(input.path(), image::ImageFormat::Png)?;
    Ok(library
        .import_staged(
            input,
            ImportOptions {
                original_name: format!("{name}.png"),
                title: Some(name.into()),
                collection: None,
            },
        )?
        .wait()
        .await?
        .sticker)
}
pub(crate) async fn open(root: &Path) -> memedock_core::Result<Library> {
    Library::open(LibraryConfig::new(
        root.join("data"),
        root.join("cache"),
        root.join("export"),
    ))
    .await
}
async fn drain(coordinator: &Rc<Coordinator>) -> TestResult {
    for _ in 0..32 {
        wait_until(|| !coordinator.snapshot().busy).await?;
        coordinator.synchronize();
        wait_until(|| !coordinator.snapshot().busy).await?;
        if coordinator.snapshot().status.pending == 0 {
            return Ok(());
        }
    }
    Err(io::Error::other("sync queue did not drain").into())
}

struct Fixture {
    child: Child,
    _log: tempfile::NamedTempFile,
    origin: String,
    first: String,
    second: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Fixture {
    fn start() -> Result<Self, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = Process::new("cargo")
            .args([
                "test",
                "--locked",
                "-p",
                "memedock-server",
                "--test",
                "auth_devices",
                "--no-run",
                "--message-format=json",
            ])
            .current_dir(&root)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        let binary = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find_map(|value| {
                (value["target"]["name"] == "auth_devices")
                    .then(|| value["executable"].as_str().map(str::to_owned))
                    .flatten()
            })
            .ok_or("server test executable missing")?;
        let log = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        let child = Process::new(binary)
            .args(["browser_fixture", "--ignored", "--exact", "--nocapture"])
            .stdout(Stdio::from(log.reopen().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(log.reopen().map_err(|e| e.to_string())?))
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut fixture = Self {
            child,
            _log: log,
            origin: "http://127.0.0.1:18787".into(),
            first: String::new(),
            second: String::new(),
        };
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let (first, second) = runtime.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .map_err(|e| e.to_string())?;
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if fixture
                    .child
                    .try_wait()
                    .map_err(|e| e.to_string())?
                    .is_some()
                {
                    return Err("server fixture stopped".into());
                }
                if client
                    .get(format!("{}/api/v1/admin/me", fixture.origin))
                    .send()
                    .await
                    .is_ok_and(|r| r.status().as_u16() == 401)
                {
                    break;
                }
                if Instant::now() > deadline {
                    return Err("server fixture startup timed out".into());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let login = client
                .post(format!("{}/api/v1/admin/login", fixture.origin))
                .header("Origin", &fixture.origin)
                .json(&serde_json::json!({"username":"admin", "password":"memedock-test-password"}))
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let cookie = login
                .headers()
                .get("set-cookie")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .ok_or("missing test session")?
                .to_owned();
            let mut tokens = Vec::new();
            for name in ["Linux A", "Linux B"] {
                let value: serde_json::Value = client
                    .post(format!("{}/api/v1/admin/devices", fixture.origin))
                    .header("Origin", &fixture.origin)
                    .header("Cookie", &cookie)
                    .json(&serde_json::json!({"name":name}))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                tokens.push(
                    value["data"]["token"]
                        .as_str()
                        .ok_or("missing device token")?
                        .to_owned(),
                );
            }
            Ok::<_, String>((tokens.remove(0), tokens.remove(0)))
        })?;
        fixture.first = first;
        fixture.second = second;
        Ok(fixture)
    }
}

/// Called by the existing single-thread GTK test, so GTK stays on its initialization thread.
pub(crate) async fn verify(parent: &impl IsA<gtk4::Widget>) -> TestResult {
    let fixture = gio::spawn_blocking(Fixture::start)
        .await
        .map_err(|_| io::Error::other("server fixture setup panicked"))?
        .map_err(io::Error::other)?;
    let root = tempfile::tempdir()?;
    let a = open(&root.path().join("a")).await?;
    let b = open(&root.path().join("b")).await?;
    let credentials = root.path().join("credentials");
    let first = Coordinator::new(a.clone(), Some(credentials.clone()));
    let second = Coordinator::new(b.clone(), Some(credentials.clone()));
    let result = async {
        wait_until(|| {
            first.snapshot().loaded
                && !first.snapshot().busy
                && second.snapshot().loaded
                && !second.snapshot().busy
        })
        .await?;
        let imported = import(&a, "Linux original", 17).await?;
        let dialog = present(parent, Rc::clone(&first));
        named(&dialog, "sync-server")
            .ok_or("server entry missing")?
            .downcast::<libadwaita::EntryRow>()
            .map_err(|_| "wrong server widget")?
            .set_text(&fixture.origin);
        let token = named(&dialog, "sync-token")
            .ok_or("token entry missing")?
            .downcast::<libadwaita::PasswordEntryRow>()
            .map_err(|_| "wrong token widget")?;
        token.set_text(&fixture.first);
        named(&dialog, "sync-connect")
            .ok_or("connect button missing")?
            .downcast::<gtk4::Button>()
            .map_err(|_| "wrong connect widget")?
            .emit_clicked();
        assert!(token.text().is_empty());
        wait_until(|| {
            first.snapshot().status.last_success_ms.is_some()
                && !first.snapshot().busy
                && first.snapshot().status.pending == 0
        })
        .await?;
        second.connect(fixture.origin.clone(), fixture.second.clone());
        wait_until(|| {
            second.snapshot().status.last_success_ms.is_some()
                && !second.snapshot().busy
                && second.snapshot().status.pending == 0
        })
        .await?;
        let detail = b.sticker_detail(imported.id())?.wait().await?;
        assert_eq!(detail.sticker.title(), "Linux original");
        assert!(detail.original_path.is_some());
        let current = a.sticker_detail(imported.id())?.wait().await?.sticker;
        let deleted = a
            .delete_sticker(current.id(), current.lifecycle().generation())?
            .wait()
            .await?;
        let purge = crate::trash::Coordinator::new(a.clone());
        purge.request(
            parent,
            vec![crate::trash::sticker(
                deleted.id(),
                deleted.lifecycle().generation(),
                deleted.lifecycle().revision(),
            )],
            Rc::new(|_| {}),
            Rc::new(|_| {}),
        );
        crate::trash::tests::accept(&purge)?;
        wait_until(|| !purge.busy()).await?;
        drain(&first).await?;
        drain(&second).await?;
        assert!(
            b.sticker_detail(imported.id())?
                .wait()
                .await?
                .sticker
                .lifecycle()
                .is_purged()
        );
        let fresh = import(&b, "Linux fresh", 17).await?;
        assert_eq!(
            fresh.lifecycle().generation().get(),
            current.lifecycle().generation().get() + 1
        );
        drain(&second).await?;
        drain(&first).await?;
        assert_eq!(
            a.sticker_detail(imported.id())?
                .wait()
                .await?
                .sticker
                .title(),
            "Linux fresh"
        );
        first.pause(true);
        wait_until(|| !first.snapshot().busy && first.snapshot().status.paused).await?;
        first.suspend().await;
        first.synchronize();
        assert!(first.commands.borrow().is_empty());
        assert!(first.current.borrow().is_none());
        first.resume();
        wait_until(|| !first.snapshot().busy).await?;
        assert!(first.snapshot().status.paused);
        assert!(first.snapshot().credential);
        first.disconnect();
        wait_until(|| !first.snapshot().busy && !first.snapshot().credential).await?;
        let store = Store::sync(
            &a.identity().library_id.to_string(),
            &fixture.origin,
            Some(&credentials),
        )
        .await
        .map_err(|_| "credential store")?;
        assert!(
            store
                .load()
                .await
                .map_err(|_| "load cleared token")?
                .is_none()
        );
        dialog.force_close();
        purge.shutdown().await;
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    first.shutdown().await;
    second.shutdown().await;
    a.close().await?;
    b.close().await?;
    result
}
