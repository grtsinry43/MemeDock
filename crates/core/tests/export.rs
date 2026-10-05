mod common;
use common::*;
use memedock_core::{ErrorCode, Library};

#[tokio::test]
async fn original_export_reuses_verified_bytes_and_survives_restart() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let bytes = b"original exact bytes";
    let (_, sticker) = seed(&configuration, bytes).await?;
    let library = Library::open(configuration.clone()).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    let b = library.export_original(sticker.id())?.wait().await?;
    assert_eq!(a.metadata().id, b.metadata().id);
    assert_eq!(std::fs::read(&a.metadata().path)?, bytes);
    let prepared = library.prepare_handoff(a.clone())?.wait().await?;
    assert!(prepared.retained_until >= a.metadata().retained_until);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    let old = a.metadata().id;
    drop(a);
    drop(b);
    library.close().await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    assert_eq!(a.metadata().id, old);
    assert_eq!(std::fs::read(&a.metadata().path)?, bytes);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn active_lease_prevents_expired_cleanup_and_corrupt_original_cannot_publish() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let (_, sticker) = seed(&configuration, b"protected").await?;
    let library = Library::open(configuration.clone()).await?;
    let lease = library.export_original(sticker.id())?.wait().await?;
    let path = lease.metadata().path.clone();
    let db = memedock_storage::LibraryDatabase::open(configuration.data_dir.join("library.sqlite"))
        .await?;
    let mut record = db
        .artifact_for(sticker.id().content_hash())
        .await?
        .ok_or("artifact")?;
    record.retained_until = 0;
    db.save_artifact(&record).await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    drop(lease);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 1);
    assert!(!path.exists());
    let blobs = memedock_storage::files::FsBlobStore::open(&configuration.data_dir)?;
    std::fs::write(
        blobs.original_path(sticker.id().content_hash()),
        b"corrupted",
    )?;
    assert_eq!(
        library
            .export_original(sticker.id())?
            .wait()
            .await
            .err()
            .ok_or("expected corruption")?
            .code(),
        ErrorCode::CorruptData
    );
    db.close().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_exports_share_one_output_and_quota_never_evicts_retained_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut configuration = config(dir.path());
    configuration.limits.export_budget_bytes = 16;
    let (_, first) = seed(&configuration, b"first original").await?;
    let (_, second) = seed(&configuration, b"second original").await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(first.id())?;
    let b = library.export_original(first.id())?;
    let (a, b) = tokio::join!(a.wait(), b.wait());
    let a = a?;
    let b = b?;
    assert_eq!(a.metadata().id, b.metadata().id);
    assert_eq!(
        library
            .export_original(second.id())?
            .wait()
            .await
            .err()
            .ok_or("expected quota")?
            .code(),
        ErrorCode::ResourceLimit
    );
    assert!(a.metadata().path.is_file());
    drop(a);
    drop(b);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn corrupt_output_is_replaced_with_new_uri_and_retired_file_keeps_retention() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let bytes = b"original exact bytes";
    let (_, sticker) = seed(&configuration, bytes).await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    let path = a.metadata().path.clone();
    let id = a.metadata().id;
    std::fs::write(&path, b"broken")?;
    assert_eq!(
        library
            .export_original(sticker.id())?
            .wait()
            .await
            .err()
            .ok_or("expected active corruption")?
            .code(),
        ErrorCode::CorruptData
    );
    drop(a);
    let b = library.export_original(sticker.id())?.wait().await?;
    assert_ne!(b.metadata().id, id);
    assert_eq!(std::fs::read(&b.metadata().path)?, bytes);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    library.close().await?;
    Ok(())
}
