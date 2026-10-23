//! Phases orchestrated by tools/testing/android-linux-sync.py against an actual Android app.
use super::{
    Coordinator,
    tests::{import, open, wait_until},
};
use gtk4::prelude::*;
use libadwaita as adw;
use memedock_core::{Library, RestoreMode};
use memedock_domain::{
    change::{FieldPatch, StickerPatch},
    identity::StickerId,
    tag::Name,
};
use serde_json::Value;
use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn field<'a>(config: &'a Value, key: &str) -> Result<&'a str> {
    config[key]
        .as_str()
        .ok_or_else(|| format!("missing {key}").into())
}
async fn settle(sync: &Rc<Coordinator>) -> Result {
    wait_until(|| !sync.snapshot().busy && sync.snapshot().loaded).await?;
    for _ in 0..32 {
        sync.synchronize();
        wait_until(|| !sync.snapshot().busy).await?;
        let state = sync.snapshot();
        assert!(state.error.is_none(), "sync failed: {:?}", state.error);
        assert_eq!(state.status.blocked, 0);
        if state.status.pending == 0 {
            return Ok(());
        }
    }
    Err("sync did not drain".into())
}
async fn pause(sync: &Rc<Coordinator>, value: bool) -> Result {
    wait_until(|| !sync.snapshot().busy).await?;
    sync.pause(value);
    wait_until(|| !sync.snapshot().busy && sync.snapshot().status.paused == value).await?;
    Ok(())
}
async fn patch(
    library: &Library,
    id: StickerId,
    title: Option<&str>,
    note: Option<&str>,
) -> Result {
    let current = library.sticker_detail(id)?.wait().await?.sticker;
    library
        .patch_sticker(
            id,
            current.lifecycle().generation(),
            StickerPatch::new(
                title.map_or(FieldPatch::Missing, |s| FieldPatch::Set(s.into())),
                note.map_or(FieldPatch::Missing, |s| FieldPatch::Set(s.into())),
                FieldPatch::Missing,
            )?,
        )?
        .wait()
        .await?;
    Ok(())
}

pub(crate) async fn verify(path: &Path) -> Result {
    let mut config: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let root = PathBuf::from(field(&config, "root")?);
    let phase = field(&config, "phase")?.to_owned();
    let library = open(&root.join("linux")).await?;
    let sync = Coordinator::new(library.clone(), Some(root.join("credentials")));
    let window = adw::Window::new();
    window.set_default_size(960, 720);
    window.present();
    let result = async {
        wait_until(|| sync.snapshot().loaded && (phase == "interrupt" || !sync.snapshot().busy))
            .await?;
        if phase == "seed" || phase == "reauthorize" {
            sync.connect(
                field(&config, "origin")?.into(),
                field(&config, "linuxToken")?.into(),
            );
            wait_until(|| !sync.snapshot().busy).await?;
            assert!(
                sync.snapshot().credential,
                "connection failed: {:?}, {:?}",
                sync.snapshot().error,
                sync.snapshot().status
            );
        } else {
            assert!(
                sync.snapshot().credential,
                "credential must survive process restart"
            );
        }
        if !matches!(
            phase.as_str(),
            "offline" | "merge" | "revoked" | "interrupt"
        ) {
            settle(&sync).await?;
        }
        if phase == "seed" {
            let sticker = import(&library, "cross-linux", 113).await?;
            let collection = library
                .create_collection(Name::new("cross-collection".into())?, None)?
                .wait()
                .await?;
            let tag = library
                .create_tag(Name::new("cross-tag".into())?)?
                .wait()
                .await?;
            library
                .set_sticker_organization(
                    sticker.id(),
                    sticker.lifecycle().generation(),
                    Some(Some((collection.id(), collection.lifecycle().generation()))),
                    Some(vec![(tag.id(), tag.lifecycle().generation())]),
                )?
                .wait()
                .await?;
            let original = library
                .sticker_detail(sticker.id())?
                .wait()
                .await?
                .original_path
                .ok_or("missing original")?;
            std::fs::copy(original, root.join("seed.png"))?;
            config["sticker"] = sticker.id().to_string().into();
            config["generation"] = sticker.lifecycle().generation().get().into();
            settle(&sync).await?;
        } else {
            let id: StickerId = field(&config, "sticker")?.parse()?;
            match phase.as_str() {
                "edited" => {
                    let detail = library.sticker_detail(id)?.wait().await?;
                    assert_eq!(detail.sticker.title(), "android-edited");
                    assert!(detail.sticker.starred());
                    assert_eq!(detail.tags.len(), 1);
                    assert!(detail.collection.is_some());
                    let android: StickerId = field(&config, "androidSticker")?.parse()?;
                    assert!(
                        library
                            .sticker_detail(android)?
                            .wait()
                            .await?
                            .original_path
                            .is_some()
                    );
                }
                "offline" => {
                    pause(&sync, true).await?;
                    patch(&library, id, Some("linux-offline"), None).await?;
                }
                "merge" => {
                    pause(&sync, false).await?;
                    settle(&sync).await?;
                    let detail = library.sticker_detail(id)?.wait().await?;
                    assert_eq!(detail.sticker.title(), "linux-offline");
                    assert_eq!(detail.sticker.note(), "android-offline");
                }
                "delete" | "purge" => {
                    // Keep the confirmed local revisions fixed until deletion;
                    // reconciliation assigning server revisions is tested separately.
                    if phase == "purge" {
                        pause(&sync, true).await?;
                    }
                    let detail = library.sticker_detail(id)?.wait().await?;
                    if phase == "purge" {
                        config["generation"] = detail.sticker.lifecycle().generation().get().into();
                    }
                    library
                        .delete_sticker(id, detail.sticker.lifecycle().generation())?
                        .wait()
                        .await?;
                    for collection in library.collections(false)?.wait().await? {
                        library
                            .delete_collection(
                                collection.id(),
                                collection.lifecycle().generation(),
                            )?
                            .wait()
                            .await?;
                    }
                    for tag in library.tags(false)?.wait().await? {
                        library
                            .delete_tag(tag.id(), tag.lifecycle().generation())?
                            .wait()
                            .await?;
                    }
                    if phase == "purge" {
                        let purge = crate::trash::Coordinator::new(library.clone());
                        purge.empty(&window, Rc::new(|_| {}));
                        wait_until(|| crate::trash::tests::accept(&purge).is_ok()).await?;
                        wait_until(|| !purge.busy()).await?;
                        assert!(library.trash_snapshot()?.wait().await?.is_empty());
                        purge.shutdown().await;
                        pause(&sync, false).await?;
                    }
                    settle(&sync).await?;
                }
                "restored" => {
                    assert!(
                        library
                            .sticker_detail(id)?
                            .wait()
                            .await?
                            .sticker
                            .lifecycle()
                            .deleted_at()
                            .is_none()
                    );
                    assert_eq!(library.collections(false)?.wait().await?.len(), 1);
                    assert_eq!(library.tags(false)?.wait().await?.len(), 1);
                }
                "fresh" => {
                    let detail = library.sticker_detail(id)?.wait().await?;
                    assert_eq!(
                        detail.sticker.lifecycle().generation().get(),
                        config["generation"].as_i64().ok_or("generation")? + 1
                    );
                    assert!(detail.sticker.lifecycle().deleted_at().is_none());
                    assert_eq!(
                        std::fs::read(detail.original_path.ok_or("fresh original missing")?)?,
                        std::fs::read(root.join("seed.png"))?
                    );
                }
                "revoked" => {
                    sync.synchronize();
                    wait_until(|| !sync.snapshot().busy && sync.snapshot().needs_authorization())
                        .await?;
                    assert!(sync.snapshot().terminal);
                }
                "reauthorize" => {
                    settle(&sync).await?;
                    assert!(!sync.snapshot().needs_authorization());
                }
                "network-verify" => {
                    assert_eq!(
                        library.sticker_detail(id)?.wait().await?.sticker.note(),
                        field(&config, "expectedNote")?
                    );
                }
                "interrupt" => {
                    wait_until(|| sync.current.borrow().is_some()).await?;
                    sync.shutdown().await;
                    assert!(sync.current.borrow().is_none());
                    assert!(sync.runner.borrow().is_none());
                }
                "backup" => {
                    backup(&library, &sync, &root).await?;
                }
                "export-backup" => {
                    let file = library.create_backup()?.wait().await?;
                    std::fs::copy(&file.path, root.join("linux-backup.memedock"))?;
                    library.discard_backup(file)?.wait().await?;
                }
                other => return Err(format!("unknown Linux phase {other}").into()),
            }
        }
        std::fs::write(path, serde_json::to_vec(&config)?)?;
        Ok(())
    }
    .await;
    sync.shutdown().await;
    window.destroy();
    library.close().await?;
    result
}

async fn backup(library: &Library, sync: &Rc<Coordinator>, root: &Path) -> Result {
    let bytes = std::fs::read(root.join("android-backup.memedock"))?;
    sync.suspend().await;
    let input = library.create_archive_input()?.wait().await?;
    std::fs::write(input.path(), &bytes)?;
    let preview = library.inspect_archive(input)?.wait().await?;
    let restored = library
        .restore_archive(preview.clone(), RestoreMode::Merge)?
        .wait()
        .await?;
    library.complete_restore(restored).await?;
    library.discard_prepared_archive(preview)?.wait().await?;
    sync.resume();
    settle(sync).await?;
    assert!(sync.snapshot().credential);
    sync.shutdown().await;
    let input = library.create_archive_input()?.wait().await?;
    std::fs::write(input.path(), bytes)?;
    let preview = library.inspect_archive(input)?.wait().await?;
    let restored = library
        .restore_archive(preview, RestoreMode::Replace)?
        .wait()
        .await?;
    library.complete_restore(restored).await?;
    library.close().await?;
    let reopened = open(&root.join("linux")).await?;
    let replacement = Coordinator::new(reopened.clone(), Some(root.join("credentials")));
    wait_until(|| replacement.snapshot().loaded && !replacement.snapshot().busy).await?;
    assert_ne!(
        reopened.identity().library_id,
        library.identity().library_id
    );
    assert!(!replacement.snapshot().credential);
    assert!(!replacement.snapshot().status.connected);
    replacement.shutdown().await;
    reopened.close().await?;
    Ok(())
}
