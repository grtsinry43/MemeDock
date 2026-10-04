mod common;
use common::*;
use memedock_core::{
    ErrorCode, Library, QueryRequest, RequestId, StickerQuery,
    events::{ChangeKind, Notification},
    tasks::Priority,
};
use memedock_domain::local::UsageAction;
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

#[tokio::test]
async fn usage_is_serial_atomic_and_notified_after_commit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let (_, sticker) = seed(&settings, b"usage").await?;
    let library = Library::open(settings.clone()).await?;
    let mut subscription = library.subscribe()?;
    let mut tasks = Vec::new();
    for _ in 0..20 {
        tasks.push(library.record_use(sticker.id(), UsageAction::ShareLaunched)?);
    }
    let mut counts = Vec::new();
    for task in tasks {
        counts.push(task.wait().await?.use_count());
    }
    counts.sort_unstable();
    assert_eq!(counts, (1..=20).collect::<Vec<_>>());
    for sequence in 1..=20 {
        let Notification::Changed(event) = subscription.next().await? else {
            return Err("missing usage event".into());
        };
        assert_eq!(event.sequence, sequence);
        assert_eq!(event.kind, ChangeKind::UsageChanged(sticker.id()));
    }
    let checkpoint = library.create_checkpoint()?.wait().await?;
    let db = memedock_storage::LibraryDatabase::open(checkpoint).await?;
    assert_eq!(
        db.local_usage(sticker.id())
            .await?
            .ok_or("usage")?
            .use_count(),
        20
    );
    assert_eq!(db.changes_after(None, 10).await?.len(), 1);
    db.close().await?;
    library.close().await?;
    assert_eq!(subscription.next().await?, Notification::Closed);
    let db =
        memedock_storage::LibraryDatabase::open(settings.data_dir.join("library.sqlite")).await?;
    assert_eq!(
        db.local_usage(sticker.id())
            .await?
            .ok_or("persisted usage")?
            .use_count(),
        20
    );
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn lagged_subscriptions_reload_and_closed_slots_are_reusable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut settings = config(dir.path());
    settings.limits.event_capacity = 2;
    let (_, sticker) = seed(&settings, b"events").await?;
    let library = Library::open(settings).await?;
    let mut slow = library.subscribe()?;
    let mut other = library.subscribe()?;
    assert_eq!(
        library
            .subscribe()
            .err()
            .ok_or("subscription limit")?
            .code(),
        ErrorCode::Busy
    );
    other.close();
    let mut replacement = library.subscribe()?;
    replacement.close();
    for _ in 0..4 {
        library
            .record_use(sticker.id(), UsageAction::CopyFile)?
            .wait()
            .await?;
    }
    assert_eq!(
        slow.next().await?,
        Notification::ReloadRequired {
            through_sequence: 4
        }
    );
    library
        .record_use(sticker.id(), UsageAction::CopyImage)?
        .wait()
        .await?;
    let Notification::Changed(event) = slow.next().await? else {
        return Err("event after reload".into());
    };
    assert_eq!(event.sequence, 5);
    slow.close();
    library.close().await?;
    Ok(())
}
#[tokio::test]
async fn queries_preserve_request_identity_and_cursor_library_ownership() -> TestResult {
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    let settings = config(a.path());
    seed(&settings, b"first").await?;
    seed(&settings, b"second").await?;
    let library = Library::open(settings).await?;
    let other = Library::open(config(b.path())).await?;
    let request_id = RequestId::new();
    let response = library
        .list_stickers(QueryRequest {
            request_id,
            query: StickerQuery::default(),
            page_size: 1,
            cursor: None,
        })?
        .wait()
        .await?;
    assert_eq!(response.request_id, request_id);
    assert_eq!(request_id.to_string().parse::<RequestId>()?, request_id);
    let cursor = response.next.ok_or("cursor")?;
    assert_eq!(
        other
            .list_stickers(QueryRequest {
                request_id,
                query: StickerQuery::default(),
                page_size: 1,
                cursor: Some(cursor.clone())
            })
            .err()
            .ok_or("foreign cursor")?
            .code(),
        ErrorCode::InvalidInput
    );
    assert_eq!(
        library
            .list_stickers(QueryRequest {
                request_id,
                query: StickerQuery::default(),
                page_size: 1,
                cursor: Some(cursor)
            })?
            .wait()
            .await?
            .stickers
            .len(),
        1
    );
    assert!(library.collections(false)?.wait().await?.is_empty());
    assert!(library.tags(false)?.wait().await?.is_empty());
    library.close().await?;
    other.close().await?;
    Ok(())
}

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
/// Minimal std-only executor: deliberately supplies no Tokio context.
fn outside_tokio<F: Future>(future: F) -> TestResult<F::Output> {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return Ok(output),
            Poll::Pending => {
                if Instant::now() > deadline {
                    return Err("external executor timeout".into());
                }
                std::thread::park_timeout(Duration::from_millis(10));
            }
        }
    }
}
#[test]
fn core_open_queries_blocking_work_and_close_work_without_tokio_on_the_caller() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (asset, sticker) = runtime.block_on(seed(&settings, b"external executor"))?;
    drop(runtime);
    assert!(tokio::runtime::Handle::try_current().is_err());
    let library = outside_tokio(Library::open(settings))??;
    assert_eq!(
        outside_tokio(library.sticker_detail(sticker.id())?.wait())??.sticker,
        sticker
    );
    assert_eq!(
        outside_tokio(
            library
                .verify_original(asset.hash(), Priority::Background)?
                .wait()
        )??
        .hash,
        asset.hash()
    );
    assert_eq!(
        outside_tokio(
            library
                .record_use(sticker.id(), UsageAction::CopyImage)?
                .wait()
        )??
        .use_count(),
        1
    );
    let path = outside_tokio(library.create_checkpoint()?.wait())??;
    assert!(path.is_file());
    outside_tokio(library.close())??;
    Ok(())
}
