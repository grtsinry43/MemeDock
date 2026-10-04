mod common;
use common::*;
use memedock_core::{
    ErrorCode, Library,
    tasks::{CancelResult, Priority, TaskId, TaskStatus},
};
use memedock_domain::identity::ContentHash;

#[tokio::test]
async fn original_integrity_failures_preserve_diagnostic_sources() -> TestResult {
    use memedock_storage::files::FsBlobStore;
    use std::error::Error;
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let (asset, _) = seed(&settings, b"original").await?;
    let store = FsBlobStore::open(&settings.data_dir)?;
    let path = store.original_path(asset.hash());
    // Deliberate external corruption of the fixture, never a core mutation.
    std::fs::write(&path, b"tampered")?;
    let library = Library::open(settings).await?;
    let error = library
        .verify_original(asset.hash(), Priority::Visible)?
        .wait()
        .await
        .err()
        .ok_or("corruption")?;
    assert_eq!(error.code(), ErrorCode::CorruptData);
    assert!(error.source().is_some());
    std::fs::remove_file(&path)?;
    let error = library
        .verify_original(asset.hash(), Priority::Visible)?
        .wait()
        .await
        .err()
        .ok_or("missing file")?;
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert!(error.source().is_some());
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn task_result_and_progress_report_real_success_and_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let (asset, _) = seed(&settings, b"tasks").await?;
    let library = Library::open(settings).await?;
    let task = library.verify_original(asset.hash(), Priority::Background)?;
    let mut progress = task.progress();
    let id = task.id();
    assert_eq!(id.to_string().parse::<TaskId>()?, id);
    let result = task.wait().await?;
    assert_eq!(result.byte_size, 5);
    loop {
        let value = progress.next().await.ok_or("terminal progress")?;
        if value.status.is_terminal() {
            assert_eq!(value.status, TaskStatus::Succeeded);
            break;
        }
    }
    let task = library.verify_original(ContentHash::from_bytes([1; 32]), Priority::Interactive)?;
    let mut progress = task.progress();
    assert_eq!(
        task.wait().await.err().ok_or("missing asset")?.code(),
        ErrorCode::NotFound
    );
    loop {
        let value = progress.next().await.ok_or("terminal progress")?;
        if value.status.is_terminal() {
            assert_eq!(value.status, TaskStatus::Failed(ErrorCode::NotFound));
            break;
        }
    }
    assert!(progress.next().await.is_none());
    assert_eq!(
        library.cancel_task("019a1c00-0000-7000-8000-000000000001".parse()?)?,
        CancelResult::NotFound
    );
    assert!(
        "00000000-0000-0000-0000-000000000000"
            .parse::<TaskId>()
            .is_err()
    );
    library.close().await?;
    Ok(())
}
