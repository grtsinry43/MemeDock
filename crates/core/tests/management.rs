mod common;
use common::*;
use memedock_core::{ErrorCode, Library, StickerQuery};
use memedock_domain::{
    change::{FieldPatch, StickerPatch},
    identity::TagId,
    tag::Name,
    version::Generation,
};
use memedock_storage::LibraryDatabase;

#[tokio::test]
async fn edits_relations_and_search_commit_together_and_survive_restart() -> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, s) = seed(&config, b"immutable management original").await?;
    let library = Library::open(config.clone()).await?;
    let c = library
        .create_collection(Name::new("猫猫".into())?, None)?
        .wait()
        .await?;
    let t = library
        .create_tag(Name::new("工作".into())?)?
        .wait()
        .await?;
    library
        .set_sticker_relations(
            s.id(),
            Generation::INITIAL,
            Some(vec![(c.id(), Generation::INITIAL)]),
            Some(vec![(t.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    let edited = library
        .patch_sticker(
            s.id(),
            Generation::INITIAL,
            StickerPatch::new(
                FieldPatch::Set("摸鱼".into()),
                FieldPatch::Set("明天再说".into()),
                FieldPatch::Set(true),
            )?,
        )?
        .wait()
        .await?;
    assert_eq!(edited.original_name(), "cat.png");
    let page = library.list_stickers(memedock_core::QueryRequest {
        request_id: memedock_core::RequestId::new(),
        query: StickerQuery {
            text: "工作 明天".into(),
            starred: Some(true),
            ..Default::default()
        },
        page_size: 60,
        cursor: None,
    })?;
    assert_eq!(page.wait().await?.stickers.len(), 1);
    let error = library
        .set_sticker_relations(
            s.id(),
            Generation::INITIAL,
            Some(vec![]),
            Some(vec![(TagId::new(), Generation::INITIAL)]),
        )?
        .wait()
        .await
        .err()
        .ok_or("error expected")?;
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert_eq!(
        library
            .sticker_detail(s.id())?
            .wait()
            .await?
            .collections
            .len(),
        1
    );
    library.close().await?;
    let db = LibraryDatabase::open(config.data_dir.join("library.sqlite")).await?;
    assert_eq!(db.changes_after(None, 100).await?.len(), 6);
    db.close().await?;
    let reopened = Library::open(config).await?;
    let detail = reopened.sticker_detail(s.id())?.wait().await?;
    assert!(detail.sticker.starred());
    assert_eq!(detail.sticker.note(), "明天再说");
    assert_eq!(detail.tags[0].id(), t.id());
    reopened.close().await?;
    Ok(())
}
