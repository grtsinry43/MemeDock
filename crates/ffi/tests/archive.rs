use memedock_ffi::*;
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
// Foreign async executors do not supply a Tokio reactor. Exercise that boundary.
fn outside_tokio<F: Future>(future: F) -> TestResult<F::Output> {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return Ok(value),
            Poll::Pending => {
                if Instant::now() > deadline {
                    return Err("foreign executor timeout".into());
                }
                std::thread::park_timeout(Duration::from_millis(10));
            }
        }
    }
}

#[test]
fn archive_tokens_are_single_use_and_replacement_works_on_a_foreign_executor() -> TestResult {
    let root = tempfile::tempdir()?;
    let settings = LibraryConfiguration {
        data_dir: root.path().join("data").to_str().ok_or("data path")?.into(),
        cache_dir: root
            .path()
            .join("cache")
            .to_str()
            .ok_or("cache path")?
            .into(),
        export_dir: root
            .path()
            .join("export")
            .to_str()
            .ok_or("export path")?
            .into(),
        limits: default_resource_configuration()?,
    };
    assert!(tokio::runtime::Handle::try_current().is_err());
    let library = outside_tokio(open_library(settings.clone()))??;
    let tag = outside_tokio(library.create_tag("保留".into())?.await_result())??;
    let output = outside_tokio(library.create_backup()?.await_result())??;
    let bytes = std::fs::read(output.path())?;
    outside_tokio(library.discard_backup(output.clone())?.await_result())??;
    assert_eq!(
        library
            .discard_backup(output)
            .err()
            .ok_or("reused backup token")?
            .code(),
        ErrorCode::Conflict
    );
    outside_tokio(library.create_tag("本机新增".into())?.await_result())??;
    let input = outside_tokio(library.create_archive_input()?.await_result())??;
    let input_path = input.path();
    std::fs::write(&input_path, bytes)?;
    let preview = outside_tokio(library.inspect_archive(input.clone())?.await_result())??;
    assert_eq!(preview.summary().tags, 1);
    assert_eq!(
        library
            .inspect_archive(input)
            .err()
            .ok_or("reused input token")?
            .code(),
        ErrorCode::Conflict
    );
    let task = library
        .clone()
        .restore_archive(preview.clone(), ArchiveRestoreMode::Replace)?;
    assert_eq!(
        library
            .clone()
            .restore_archive(preview, ArchiveRestoreMode::Merge)
            .err()
            .ok_or("reused preview token")?
            .code(),
        ErrorCode::Conflict
    );
    outside_tokio(task.await_result())??;
    assert!(!std::path::Path::new(&input_path).exists());
    assert_eq!(
        outside_tokio(task.await_result())?
            .err()
            .ok_or("reused task result")?
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(library.state()?, LibraryState::Closed);
    let reopened = outside_tokio(open_library(settings))??;
    let tags = outside_tokio(reopened.tags(false)?.await_result())??;
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, tag.id);
    outside_tokio(reopened.shutdown())??;
    Ok(())
}
