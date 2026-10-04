mod common;
use common::*;
use memedock_core::{ErrorCode, Library, LibraryState};
use std::time::Duration;

#[tokio::test]
async fn concurrent_open_owns_one_instance_and_close_releases_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let config = config(dir.path());
    let (asset, sticker) = seed(&config, b"lifecycle").await?;
    let (a, b) = tokio::join!(Library::open(config.clone()), Library::open(config.clone()));
    let library = match (a, b) {
        (Ok(lib), Err(error)) | (Err(error), Ok(lib)) => {
            assert_eq!(error.code(), ErrorCode::AlreadyOpen);
            lib
        }
        _ => return Err("expected one successful open".into()),
    };
    assert_eq!(
        library.sticker_detail(sticker.id())?.wait().await?.asset,
        asset
    );
    let identity = library.identity();
    let clone = library.clone();
    let (a, b) = tokio::join!(library.close(), clone.close());
    a?;
    b?;
    assert_eq!(library.state()?, LibraryState::Closed);
    assert_eq!(
        library
            .space_statistics()
            .err()
            .ok_or("closed request")?
            .code(),
        ErrorCode::Closed
    );
    let reopened = Library::open(config).await?;
    assert_eq!(reopened.identity(), identity);
    reopened.close().await?;
    Ok(())
}
#[tokio::test]
async fn failed_open_releases_reservations_before_returning_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let config = config(dir.path());
    std::fs::create_dir_all(config.data_dir.join("library.sqlite"))?;
    assert!(Library::open(config.clone()).await.is_err());
    std::fs::remove_dir(config.data_dir.join("library.sqlite"))?;
    Library::open(config).await?.close().await?;
    Ok(())
}
#[tokio::test]
async fn dropping_the_last_handle_closes_without_blocking_the_caller() -> TestResult {
    let dir = tempfile::tempdir()?;
    let config = config(dir.path());
    let library = Library::open(config.clone()).await?;
    let identity = library.identity();
    drop(library);
    let reopened = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match Library::open(config.clone()).await {
                Ok(library) => return Ok(library),
                Err(e) if e.code() == ErrorCode::AlreadyOpen => {
                    tokio::time::sleep(Duration::from_millis(5)).await
                }
                Err(e) => return Err(e),
            }
        }
    })
    .await??;
    assert_eq!(reopened.identity(), identity);
    reopened.close().await?;
    Ok(())
}
#[tokio::test]
async fn directories_and_resource_limits_are_validated() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut settings = config(dir.path());
    settings.cache_dir = settings.data_dir.join("cache");
    assert_eq!(
        Library::open(settings).await.err().ok_or("overlap")?.code(),
        ErrorCode::InvalidInput
    );
    let mut settings = config(dir.path());
    settings.limits.task_capacity = 0;
    assert_eq!(
        Library::open(settings)
            .await
            .err()
            .ok_or("zero capacity")?
            .code(),
        ErrorCode::InvalidInput
    );
    #[cfg(unix)]
    {
        let other = tempfile::tempdir()?;
        std::os::unix::fs::symlink(other.path(), dir.path().join("alias"))?;
        let first = Library::open(config(dir.path())).await?;
        // A configured directory must not itself be a symbolic link.
        let mut direct = config(dir.path());
        direct.data_dir = dir.path().join("alias");
        assert_eq!(
            Library::open(direct).await.err().ok_or("symlink")?.code(),
            ErrorCode::InvalidInput
        );
        first.close().await?;
    }
    Ok(())
}
