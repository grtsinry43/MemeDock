mod common;
use common::*;
use memedock_domain::identity::OperationId;
use memedock_storage::{artifacts::ArtifactRecord, files::ExportStore};
use std::{fs::File, io::Write};

#[tokio::test]
async fn registry_and_verified_outputs_survive_restart_without_business_logs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    let db = memedock_storage::LibraryDatabase::open(&path).await?;
    let bytes = b"immutable original";
    let (asset, sticker) = fixture(bytes, "original", 100)?;
    insert(&db, &asset, &sticker).await?;
    let mut source = File::create(dir.path().join("source"))?;
    source.write_all(bytes)?;
    drop(source);
    let store = ExportStore::open(&dir.path().join("share"))?;
    let record = ArtifactRecord {
        id: OperationId::new(),
        source_hash: asset.hash(),
        format: asset.format(),
        byte_size: bytes.len() as u64,
        animated: false,
        retained_until: 123456,
        deleting: false,
    };
    store.publish(&record, File::open(dir.path().join("source"))?, || false)?;
    db.save_artifact(&record).await?;
    db.close().await?;
    let db = memedock_storage::LibraryDatabase::open(&path).await?;
    let restored = db
        .artifact_for(asset.hash())
        .await?
        .ok_or("artifact missing")?;
    assert_eq!(restored.id, record.id);
    assert_eq!(restored.retained_until, 123456);
    assert_eq!(db.changes_after(None, 10).await?.len(), 1);
    store.verify(&restored, || false)?;
    assert!(
        store
            .publish(&record, File::open(dir.path().join("source"))?, || false)
            .is_err()
    );
    assert_eq!(std::fs::read(store.path(&record))?, bytes);
    assert!(matches!(
        store.verify(&record, || true),
        Err(memedock_storage::StorageError::Cancelled)
    ));
    std::fs::write(store.path(&record), b"corrupt")?;
    assert!(store.verify(&record, || false).is_err());
    assert_eq!(db.sticker(sticker.id()).await?, Some(sticker));
    db.close().await?;
    Ok(())
}

#[test]
fn failed_publication_and_old_orphan_scan_preserve_unrelated_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = ExportStore::open(&dir.path().join("share"))?;
    let bytes = b"fixture";
    let (asset, _) = fixture(bytes, "original", 100)?;
    let source = dir.path().join("source");
    std::fs::write(&source, bytes)?;
    let record = ArtifactRecord {
        id: OperationId::new(),
        source_hash: asset.hash(),
        format: asset.format(),
        byte_size: bytes.len() as u64,
        animated: false,
        retained_until: 0,
        deleting: false,
    };
    assert!(matches!(
        store.publish(&record, File::open(&source)?, || true),
        Err(memedock_storage::StorageError::Cancelled)
    ));
    assert!(!store.path(&record).exists());
    store.publish(&record, File::open(&source)?, || false)?;
    let output = File::open(store.path(&record))?;
    output.set_times(
        std::fs::FileTimes::new()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1)),
    )?;
    std::fs::write(dir.path().join("share/unknown.txt"), b"keep")?;
    let orphans = store.orphan_page(None, 2000)?;
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].id, record.id);
    store.remove_orphan(&orphans[0])?;
    assert!(dir.path().join("share/unknown.txt").exists());
    Ok(())
}
