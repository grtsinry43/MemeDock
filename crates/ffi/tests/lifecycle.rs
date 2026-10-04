mod common;
use common::*;
use memedock_ffi::*;

#[tokio::test]
async fn opaque_cursors_bind_library_and_query_and_shutdown_releases_ownership() -> TestResult {
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    let settings = config(a.path())?;
    seed(&settings, b"first").await?;
    seed(&settings, b"second").await?;
    let library = open_library(settings.clone()).await?;
    assert_eq!(
        open_library(settings.clone())
            .await
            .err()
            .ok_or("duplicate open")?
            .code(),
        ErrorCode::AlreadyOpen
    );
    let other = open_library(config(b.path())?).await?;
    let page = library.list_stickers(query())?.await_result().await?;
    let cursor = page.next.ok_or("cursor")?;
    let mut foreign = query();
    foreign.cursor = Some(cursor.clone());
    assert_eq!(
        other
            .list_stickers(foreign)
            .err()
            .ok_or("foreign cursor")?
            .code(),
        ErrorCode::InvalidInput
    );
    let mut changed = query();
    changed.text = "changed".into();
    changed.cursor = Some(cursor.clone());
    assert_eq!(
        library
            .list_stickers(changed)?
            .await_result()
            .await
            .err()
            .ok_or("query cursor")?
            .code(),
        ErrorCode::InvalidInput
    );
    let mut next = query();
    next.cursor = Some(cursor);
    assert_eq!(
        library
            .list_stickers(next)?
            .await_result()
            .await?
            .stickers
            .len(),
        1
    );
    let identity = library.identity();
    let subscription = library.subscribe()?;
    library.shutdown().await?;
    library.shutdown().await?;
    assert_eq!(library.state()?, LibraryState::Closed);
    assert_eq!(subscription.next().await?, Notification::Closed);
    assert_eq!(
        library.space_statistics().err().ok_or("closed")?.code(),
        ErrorCode::Closed
    );
    let reopened = open_library(settings).await?;
    assert_eq!(reopened.identity(), identity);
    reopened.shutdown().await?;
    other.shutdown().await?;
    Ok(())
}
