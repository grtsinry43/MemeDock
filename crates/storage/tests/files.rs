use memedock_domain::identity::ContentHash;
use memedock_storage::{
    StorageError,
    files::{FsBlobStore, PublishDisposition},
};
use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{self, Read},
    sync::Barrier,
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn cache_usage_counts_only_direct_regular_files_and_handles_eviction() -> TestResult {
    use memedock_storage::files::{DerivedStore, ExportStore};
    let directory = tempfile::tempdir()?;
    let derived = DerivedStore::open(directory.path())?;
    let exports_path = directory.path().join("exports");
    let exports = ExportStore::open(&exports_path)?;
    let thumbnails = directory.path().join("thumbnails");
    assert_eq!(derived.bytes_used(|| false)?, 0);
    assert_eq!(exports.bytes_used()?, 0);
    let thumbnail = thumbnails.join("sample.png");
    let export = exports_path.join("sample.png");
    fs::write(&thumbnail, b"thumbnail")?;
    fs::write(&export, b"share")?;
    fs::write(directory.path().join("unrelated"), b"not cache")?;
    fs::create_dir(thumbnails.join("nested"))?;
    fs::write(thumbnails.join("nested/ignored"), b"not a thumbnail")?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&export, thumbnails.join("file-link"))?;
        std::os::unix::fs::symlink(&thumbnail, exports_path.join("file-link"))?;
        std::os::unix::fs::symlink(&exports_path, thumbnails.join("directory-link"))?;
    }
    assert_eq!(derived.bytes_used(|| false)?, 9);
    assert_eq!(exports.bytes_used()?, 5);
    assert!(matches!(
        derived.bytes_used(|| true),
        Err(StorageError::Cancelled)
    ));
    assert!(matches!(
        exports.bytes_used_cancellable(|| true),
        Err(StorageError::Cancelled)
    ));

    // Remove the file after directory iteration starts, before metadata is read.
    let mut checks = 0;
    let mut removal = None;
    assert_eq!(
        exports.bytes_used_cancellable(|| {
            checks += 1;
            if checks == 2 {
                removal = Some(fs::remove_file(&export));
            }
            false
        })?,
        0
    );
    removal.ok_or("eviction callback did not run")??;
    assert!(!export.exists());
    fs::remove_dir_all(&thumbnails)?;
    assert_eq!(derived.bytes_used(|| false)?, 0);
    Ok(())
}

#[test]
fn platform_staging_is_rehashed_and_modified_candidates_cannot_publish() -> TestResult {
    let directory = tempfile::tempdir()?;
    let store = FsBlobStore::open(directory.path())?;
    let pending = store.create_staging()?;
    fs::write(pending.path(), b"validated bytes")?;
    let staged = pending.finish(100, || false)?;
    fs::write(pending.path(), b"different bytes")?;
    assert!(matches!(
        store.publish(staged, || false),
        Err(StorageError::Integrity(_))
    ));
    pending.discard()?;
    assert!(store.scan_recovery(&HashSet::new())?.staging.is_empty());
    Ok(())
}

#[test]
fn failed_derived_encoding_preserves_ready_file_and_recovery_keeps_unknowns() -> TestResult {
    use memedock_storage::files::DerivedStore;
    use std::io::Write;
    let directory = tempfile::tempdir()?;
    let store = DerivedStore::open(directory.path())?;
    let hash = ContentHash::from_bytes([7; 32]);
    let destination = store.thumbnail_path(hash);
    let output = store.publish(&destination, |file| {
        file.write_all(b"verified cache")?;
        Ok(())
    })?;
    assert!(
        store
            .publish(&destination, |file| {
                file.write_all(b"partial")?;
                Err(StorageError::Integrity("encoder failed"))
            })
            .is_err()
    );
    assert_eq!(fs::read(output)?, b"verified cache");
    let abandoned = directory.path().join("thumbnails").join(format!(
        "{}.part",
        memedock_domain::identity::OperationId::new()
    ));
    fs::write(&abandoned, b"abandoned")?;
    let unknown = directory.path().join("thumbnails/keep.txt");
    fs::write(&unknown, b"unknown")?;
    store.discard_abandoned_publications()?;
    assert!(!abandoned.exists());
    assert!(unknown.exists());
    Ok(())
}

#[test]
fn bounded_staging_and_cancellation_clean_up_partial_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    assert!(matches!(
        store.stage_from(&mut b"too long".as_slice(), 3, || false),
        Err(StorageError::InvalidInput(_))
    ));
    assert!(matches!(
        store.stage_from(&mut b"input".as_slice(), 100, || true),
        Err(StorageError::Cancelled)
    ));
    assert!(store.scan_recovery(&HashSet::new())?.staging.is_empty());
    let staged = store.stage_from(&mut b"abc".as_slice(), 3, || false)?;
    assert_eq!(
        staged.hash().to_string(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    staged.discard()?;
    Ok(())
}
#[test]
fn partial_failed_input_is_cleaned_up() -> TestResult {
    struct Broken(bool);
    impl Read for Broken {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                Err(io::Error::other("source failure"))
            } else {
                self.0 = true;
                buffer[0] = 1;
                Ok(1)
            }
        }
    }
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    assert!(matches!(
        store.stage_from(&mut Broken(false), 100, || false),
        Err(StorageError::Io(_))
    ));
    assert!(store.scan_recovery(&HashSet::new())?.staging.is_empty());
    Ok(())
}
#[test]
fn concurrent_identical_publications_are_atomic_and_reuse_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    let a = store.stage_from(&mut b"same bytes".as_slice(), 100, || false)?;
    let b = store.stage_from(&mut b"same bytes".as_slice(), 100, || false)?;
    let hash = a.hash();
    let barrier = Barrier::new(2);
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            store.publish(a, || false)
        });
        let b = scope.spawn(|| {
            barrier.wait();
            store.publish(b, || false)
        });
        (a.join(), b.join())
    });
    let a = a.map_err(|_| "publisher panicked")??;
    let b = b.map_err(|_| "publisher panicked")??;
    assert_ne!(a.disposition, b.disposition);
    assert!([a.disposition, b.disposition].contains(&PublishDisposition::Created));
    store.verify(hash, 10, || false)?;
    assert!(
        store
            .scan_recovery(&HashSet::from([hash]))?
            .staging
            .is_empty()
    );
    Ok(())
}
#[test]
fn existing_corrupt_original_is_never_overwritten() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    let staged = store.stage_from(&mut b"good".as_slice(), 100, || false)?;
    let published = store.publish(staged, || false)?;
    fs::write(&published.path, b"evil")?;
    let staged = store.stage_from(&mut b"good".as_slice(), 100, || false)?;
    assert!(matches!(
        store.publish(staged, || false),
        Err(StorageError::Integrity(_))
    ));
    assert_eq!(fs::read(published.path)?, b"evil");
    assert_eq!(store.scan_recovery(&HashSet::new())?.staging.len(), 1);
    Ok(())
}
#[test]
fn recovery_reports_orphans_missing_files_and_abandoned_staging() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    let staged = store.stage_from(&mut b"original".as_slice(), 100, || false)?;
    let hash = staged.hash();
    store.publish(staged, || false)?;
    let staged = store.stage_from(&mut b"abandoned".as_slice(), 100, || false)?;
    drop(staged);
    let missing = ContentHash::from_bytes([1; 32]);
    let scan = store.scan_recovery(&HashSet::from([missing]))?;
    assert_eq!(scan.orphan_originals, vec![hash]);
    assert_eq!(scan.missing_originals, vec![missing]);
    assert_eq!(scan.staging.len(), 1);
    assert!(store.original_path(hash).exists());
    Ok(())
}
#[test]
fn publication_rejects_cross_library_ownership_and_unsafe_shards() -> TestResult {
    let dir = tempfile::tempdir()?;
    let a = FsBlobStore::open(dir.path().join("a"))?;
    let b = FsBlobStore::open(dir.path().join("b"))?;
    let staged = a.stage_from(&mut b"owned".as_slice(), 100, || false)?;
    assert!(matches!(
        b.publish(staged, || false),
        Err(StorageError::InvalidInput(_))
    ));
    assert_eq!(a.scan_recovery(&HashSet::new())?.staging.len(), 1);
    #[cfg(unix)]
    {
        let staged = a.stage_from(&mut b"unsafe".as_slice(), 100, || false)?;
        let hash = staged.hash();
        let shard = a.root().join("blobs").join(&hash.to_string()[..2]);
        std::os::unix::fs::symlink(b.root(), &shard)?;
        assert!(a.publish(staged, || false).is_err());
        assert!(
            a.scan_recovery(&HashSet::new())?
                .unexpected_paths
                .contains(&shard)
        );
    }
    Ok(())
}

#[test]
fn failed_publication_retains_staging_for_recovery() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = FsBlobStore::open(dir.path())?;
    let staged = store.stage_from(&mut b"publish failure".as_slice(), 100, || false)?;
    let hash = staged.hash();
    let shard = store.root().join("blobs").join(&hash.to_string()[..2]);
    fs::write(&shard, b"a file blocks the shard")?;
    assert!(store.publish(staged, || false).is_err());
    let scan = store.scan_recovery(&HashSet::new())?;
    assert_eq!(scan.staging.len(), 1);
    assert!(scan.unexpected_paths.contains(&shard));
    assert!(!store.original_path(hash).exists());
    Ok(())
}
