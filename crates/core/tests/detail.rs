mod common;
use common::*;
use memedock_core::{ErrorCode, Library};

#[tokio::test]
async fn detail_reports_missing_original_without_hiding_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let (_, sticker) = seed(&configuration, b"preview").await?;
    let library = Library::open(configuration).await?;
    let detail = library.sticker_detail(sticker.id())?.wait().await?;
    let path = detail.original_path.ok_or("preview path")?;
    std::fs::remove_file(path)?;
    let detail = library.sticker_detail(sticker.id())?.wait().await?;
    assert_eq!(detail.sticker, sticker);
    assert_eq!(detail.original_error, Some(ErrorCode::NotFound));
    assert!(detail.original_path.is_none());
    library.close().await?;
    Ok(())
}
