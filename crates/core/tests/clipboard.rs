mod common;
use common::*;
use memedock_core::Library;

#[tokio::test]
async fn pending_and_current_clipboard_pins_protect_expired_outputs_after_restart() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let (_, first) = seed(&settings, b"first clipboard").await?;
    let (_, second) = seed(&settings, b"second clipboard").await?;
    let library = Library::open(settings.clone()).await?;
    let lease = library.export_original(first.id())?.wait().await?;
    let path = lease.metadata().path.clone();
    let token = library.protect_clipboard(lease.clone())?.wait().await?;
    let db =
        memedock_storage::LibraryDatabase::open(settings.data_dir.join("library.sqlite")).await?;
    let mut record = db
        .artifact_by_id(lease.metadata().id)
        .await?
        .ok_or("record")?;
    record.retained_until = 0;
    db.save_artifact(&record).await?;
    drop(lease);
    library.close().await?;
    let library = Library::open(settings).await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    library.reconcile_clipboard(Some(token))?.wait().await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    let next = library.export_original(second.id())?.wait().await?;
    let failed = library.protect_clipboard(next.clone())?.wait().await?;
    library.abort_clipboard(failed)?.wait().await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    let replacement = library.protect_clipboard(next.clone())?.wait().await?;
    library
        .reconcile_clipboard(Some(replacement))?
        .wait()
        .await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 1);
    assert!(!path.exists());
    assert!(next.metadata().path.is_file());
    library.reconcile_clipboard(None)?.wait().await?;
    drop(next);
    db.close().await?;
    library.close().await?;
    Ok(())
}
