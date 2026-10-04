mod common;
use common::*;
use memedock_ffi::*;
use std::time::Duration;

#[tokio::test]
async fn task_wait_is_single_consumption_and_control_survives_completion() -> TestResult {
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path())?;
    let (asset, sticker) = seed(&settings, b"task controls").await?;
    let library = open_library(settings).await?;
    assert_eq!(
        library
            .list_stickers(query())?
            .await_result()
            .await?
            .stickers
            .len(),
        1
    );
    let task = library.verify_original(asset.hash().to_string(), Priority::Background)?;
    let progress = task.progress();
    assert_eq!(task.await_result().await?.hash, asset.hash().to_string());
    assert_eq!(task.snapshot().id, task.id());
    assert_eq!(task.cancel(), CancelResult::Finished);
    assert_eq!(
        task.await_result().await.err().ok_or("double wait")?.code(),
        ErrorCode::Conflict
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(snapshot) = progress.next().await? {
            if snapshot.status == TaskStatus::Succeeded {
                break;
            }
        }
        Result::<()>::Ok(())
    })
    .await??;
    assert!(progress.next().await?.is_none());
    let sub = library.subscribe()?;
    let usage = library
        .record_use(sticker.id().to_string(), UsageAction::ShareLaunched)?
        .await_result()
        .await?;
    assert_eq!(usage.use_count, 1);
    assert!(matches!(
        sub.next().await?,
        Notification::UsageChanged { sequence: 1, .. }
    ));
    let snapshot = library.create_checkpoint()?.await_result().await?;
    assert!(std::path::Path::new(&snapshot).is_file());
    library.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stream_cancellation_restores_receiver_and_unsubscribe_wakes_pending_reader() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path())?;
    let (_, sticker) = seed(&settings, b"stream controls").await?;
    let library = open_library(settings).await?;
    let subscription = library.subscribe()?;
    let mut waiting = Box::pin(subscription.next());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), waiting.as_mut())
            .await
            .is_err()
    );
    assert_eq!(
        subscription
            .next()
            .await
            .err()
            .ok_or("second reader")?
            .code(),
        ErrorCode::Busy
    );
    drop(waiting);
    library
        .record_use(sticker.id().to_string(), UsageAction::CopyFile)?
        .await_result()
        .await?;
    assert!(matches!(
        subscription.next().await?,
        Notification::UsageChanged { sequence: 1, .. }
    ));
    let mut waiting = Box::pin(subscription.next());
    assert!(
        tokio::time::timeout(Duration::from_millis(10), waiting.as_mut())
            .await
            .is_err()
    );
    subscription.unsubscribe()?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), waiting).await??,
        Notification::Closed
    );
    subscription.unsubscribe()?;
    library.shutdown().await?;
    Ok(())
}
