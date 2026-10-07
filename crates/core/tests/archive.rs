use memedock_core::{ErrorCode, ImportOptions, Library, LibraryConfig, RestoreMode};
use memedock_domain::{
    change::{FieldPatch, StickerPatch},
    tag::Name,
    version::Generation,
};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
    sync::Arc,
};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn config(root: &Path) -> LibraryConfig {
    LibraryConfig::new(root.join("data"), root.join("cache"), root.join("exports"))
}
fn png(color: u8) -> TestResult<Vec<u8>> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        4,
        image::Rgba([color, 20, 100, 128]),
    ));
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
async fn import(library: &Library, color: u8) -> TestResult<memedock_domain::sticker::Sticker> {
    let input = library.create_import_input()?.wait().await?;
    std::fs::write(input.path(), png(color)?)?;
    Ok(library
        .import_staged(
            input,
            ImportOptions {
                original_name: "透明贴纸.png".into(),
                ..Default::default()
            },
        )?
        .wait()
        .await?
        .sticker)
}
async fn backup(library: &Library) -> TestResult<Vec<u8>> {
    let file = library.create_backup()?.wait().await?;
    let bytes = std::fs::read(&file.path)?;
    library.discard_backup(file)?.wait().await?;
    Ok(bytes)
}
async fn inspect(
    library: &Library,
    bytes: &[u8],
) -> TestResult<Arc<memedock_core::PreparedArchive>> {
    let input = library.create_archive_input()?.wait().await?;
    std::fs::write(input.path(), bytes)?;
    Ok(library.inspect_archive(input)?.wait().await?)
}

#[tokio::test]
async fn real_archive_preserves_original_relations_deletion_and_restart() -> TestResult {
    let source = tempfile::tempdir()?;
    let library = Library::open(config(source.path())).await?;
    let active = import(&library, 10).await?;
    let deleted = import(&library, 30).await?;
    let collection = library
        .create_collection(Name::new("喜欢".into())?, None)?
        .wait()
        .await?;
    let tag = library
        .create_tag(Name::new("透明".into())?)?
        .wait()
        .await?;
    library
        .set_sticker_organization(
            active.id(),
            Generation::INITIAL,
            Some(Some((collection.id(), Generation::INITIAL))),
            Some(vec![(tag.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    library
        .set_sticker_organization(
            deleted.id(),
            Generation::INITIAL,
            Some(Some((collection.id(), Generation::INITIAL))),
            Some(vec![(tag.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    library
        .delete_sticker(deleted.id(), Generation::INITIAL)?
        .wait()
        .await?;
    let bytes = backup(&library).await?;
    library.close().await?;
    for existing in [false, true] {
        let target = tempfile::tempdir()?;
        let target_config = config(target.path());
        let restored = Library::open(target_config.clone()).await?;
        if existing {
            restored
                .create_tag(Name::new("本机已有".into())?)?
                .wait()
                .await?;
        }
        let prepared = inspect(&restored, &bytes).await?;
        assert_eq!(prepared.summary().stickers, 2);
        assert_eq!(prepared.summary().deleted_stickers, 1);
        let result = restored
            .restore_archive(prepared.clone(), RestoreMode::Merge)?
            .wait()
            .await?;
        restored.complete_restore(result).await?;
        restored.discard_prepared_archive(prepared)?.wait().await?;
        restored.close().await?;
        let restored = Library::open(target_config.clone()).await?;
        let detail = restored.sticker_detail(active.id())?.wait().await?;
        assert_eq!(
            detail
                .collection
                .as_ref()
                .ok_or("collection expected")?
                .id(),
            collection.id()
        );
        assert_eq!(detail.tags[0].id(), tag.id());
        assert_eq!(
            std::fs::read(detail.original_path.ok_or("original missing")?)?,
            png(10)?
        );
        assert!(
            !restored
                .sticker_detail(deleted.id())?
                .wait()
                .await?
                .sticker
                .lifecycle()
                .is_active()
        );
        let suggestions = restored.restore_suggestions(deleted.id())?.wait().await?;
        assert_eq!(
            suggestions
                .collection
                .as_ref()
                .ok_or("collection expected")?
                .id(),
            collection.id()
        );
        assert_eq!(suggestions.tags[0].id(), tag.id());
        restored
            .request_thumbnail(active.id(), memedock_core::tasks::Priority::Visible)?
            .wait()
            .await?;
        restored.close().await?;
        let db =
            memedock_storage::LibraryDatabase::open(target_config.data_dir.join("library.sqlite"))
                .await?;
        assert_eq!(
            db.changes_after(None, 20).await?.len(),
            if existing { 2 } else { 1 },
            "restore logs the new local operation, not the archived outbox"
        );
        db.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn merge_preserves_edits_deletions_and_removed_relations_and_rejects_stale_preview()
-> TestResult {
    let root = tempfile::tempdir()?;
    let library = Library::open(config(root.path())).await?;
    let first = import(&library, 10).await?;
    let second = import(&library, 20).await?;
    let collection = library
        .create_collection(Name::new("原合集".into())?, None)?
        .wait()
        .await?;
    library
        .set_sticker_organization(
            first.id(),
            Generation::INITIAL,
            Some(Some((collection.id(), Generation::INITIAL))),
            None,
        )?
        .wait()
        .await?;
    let bytes = backup(&library).await?;
    library
        .patch_sticker(
            first.id(),
            Generation::INITIAL,
            StickerPatch::new(
                FieldPatch::Set("本地修改".into()),
                FieldPatch::Missing,
                FieldPatch::Missing,
            )?,
        )?
        .wait()
        .await?;
    library
        .set_sticker_organization(first.id(), Generation::INITIAL, Some(None), None)?
        .wait()
        .await?;
    library
        .delete_sticker(second.id(), Generation::INITIAL)?
        .wait()
        .await?;
    let prepared = inspect(&library, &bytes).await?;
    library
        .restore_archive(prepared.clone(), RestoreMode::Merge)?
        .wait()
        .await?;
    library.discard_prepared_archive(prepared)?.wait().await?;
    let detail = library.sticker_detail(first.id())?.wait().await?;
    assert_eq!(detail.sticker.title(), "本地修改");
    assert!(detail.collection.is_none());
    assert!(
        !library
            .sticker_detail(second.id())?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .is_active()
    );
    let prepared = inspect(&library, &bytes).await?;
    library
        .create_tag(Name::new("预览后的编辑".into())?)?
        .wait()
        .await?;
    let failure = library
        .restore_archive(prepared.clone(), RestoreMode::Merge)?
        .wait()
        .await
        .err()
        .ok_or("stale preview accepted")?;
    assert_eq!(failure.code(), ErrorCode::Conflict);
    library.discard_prepared_archive(prepared)?.wait().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn replacement_preserves_checkpoint_and_clipboard_delivery_after_restart() -> TestResult {
    let source = tempfile::tempdir()?;
    let target = tempfile::tempdir()?;
    let source_library = Library::open(config(source.path())).await?;
    let archived = import(&source_library, 10).await?;
    let bytes = backup(&source_library).await?;
    source_library.close().await?;
    let target_config = config(target.path());
    let library = Library::open(target_config.clone()).await?;
    let original = import(&library, 40).await?;
    let lease = library.export_original(original.id())?.wait().await?;
    let output_path = lease.metadata().path.clone();
    let pin = library.protect_clipboard(lease.clone())?.wait().await?;
    drop(lease);
    let old_identity = library.identity();
    let prepared = inspect(&library, &bytes).await?;
    let result = library
        .restore_archive(prepared, RestoreMode::Replace)?
        .wait()
        .await?;
    let result = library.complete_restore(result).await?;
    let checkpoint = result.checkpoint.ok_or("checkpoint missing")?;
    let db = memedock_storage::LibraryDatabase::open(checkpoint).await?;
    assert_eq!(db.identity(), old_identity);
    assert!(db.sticker(original.id()).await?.is_some());
    db.close().await?;
    let reopened = Library::open(target_config).await?;
    assert_ne!(reopened.identity(), old_identity);
    assert!(
        reopened
            .sticker_detail(archived.id())?
            .wait()
            .await?
            .original_path
            .is_some()
    );
    assert_eq!(
        reopened
            .sticker_detail(original.id())?
            .wait()
            .await
            .err()
            .ok_or("old sticker retained")?
            .code(),
        ErrorCode::NotFound
    );
    reopened.reconcile_clipboard(Some(pin))?.wait().await?;
    reopened.clean_export_artifacts()?.wait().await?;
    assert!(output_path.is_file());
    assert_eq!(std::fs::read(output_path)?, png(40)?);
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn tampered_metadata_and_escaping_paths_are_rejected_without_business_changes() -> TestResult
{
    let root = tempfile::tempdir()?;
    let library = Library::open(config(root.path())).await?;
    let sticker = import(&library, 10).await?;
    let bytes = backup(&library).await?;
    for escaping in [false, true] {
        let mut reader = zip::ZipArchive::new(Cursor::new(&bytes))?;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for index in 0..reader.len() {
            let mut entry = reader.by_index(index)?;
            let name = entry.name().to_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            let output_name = if escaping && name.starts_with("originals/") {
                "../outside"
            } else {
                &name
            };
            if !escaping && name == "library.json" {
                data.push(b' ');
            }
            writer.start_file(output_name, options)?;
            writer.write_all(&data)?;
        }
        let broken = writer.finish()?.into_inner();
        let input = library.create_archive_input()?.wait().await?;
        let input_path = input.path().to_owned();
        std::fs::write(&input_path, broken)?;
        assert!(library.inspect_archive(input)?.wait().await.is_err());
        assert!(!input_path.exists());
        assert_eq!(
            library.sticker_detail(sticker.id())?.wait().await?.sticker,
            sticker
        );
    }
    library.close().await?;
    Ok(())
}
