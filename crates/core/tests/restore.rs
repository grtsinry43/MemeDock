mod common;
use common::*;
use memedock_core::{ErrorCode, Library};
use memedock_domain::{
    tag::Name,
    version::{Generation, Revision},
};

#[tokio::test]
async fn recovery_requires_original_and_new_generation_never_revives_old_relations() -> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, s) = seed(&config, b"verified restore original").await?;
    let library = Library::open(config.clone()).await?;
    let c = library
        .create_collection(Name::new("Cats".into())?, None)?
        .wait()
        .await?;
    let t = library
        .create_tag(Name::new("cute".into())?)?
        .wait()
        .await?;
    library
        .set_sticker_organization(
            s.id(),
            Generation::INITIAL,
            Some(Some((c.id(), Generation::INITIAL))),
            Some(vec![(t.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    library
        .delete_sticker(s.id(), Generation::INITIAL)?
        .wait()
        .await?;
    let suggestions = library.restore_suggestions(s.id())?.wait().await?;
    assert_eq!(
        suggestions
            .collection
            .as_ref()
            .ok_or("collection expected")?
            .id(),
        c.id()
    );
    let restored = library
        .restore_sticker(s.id(), Generation::INITIAL, Revision::LOCAL)?
        .wait()
        .await?;
    assert_eq!(restored.lifecycle().generation().get(), 1);
    let detail = library.sticker_detail(s.id())?.wait().await?;
    assert!(detail.collection.is_none() && detail.tags.is_empty());
    library
        .set_sticker_organization(
            s.id(),
            Generation::new(1)?,
            Some(Some((c.id(), Generation::INITIAL))),
            Some(vec![(t.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    library
        .delete_collection(c.id(), Generation::INITIAL)?
        .wait()
        .await?;
    library
        .restore_collection(c.id(), Generation::INITIAL, Revision::LOCAL)?
        .wait()
        .await?;
    assert!(
        library
            .sticker_detail(s.id())?
            .wait()
            .await?
            .collection
            .is_none()
    );
    library
        .delete_tag(t.id(), Generation::INITIAL)?
        .wait()
        .await?;
    library
        .restore_tag(t.id(), Generation::INITIAL, Revision::LOCAL)?
        .wait()
        .await?;
    assert!(
        library
            .sticker_detail(s.id())?
            .wait()
            .await?
            .tags
            .is_empty()
    );
    library
        .delete_sticker(s.id(), Generation::new(1)?)?
        .wait()
        .await?;
    let stale = library
        .restore_sticker(s.id(), Generation::INITIAL, Revision::LOCAL)?
        .wait()
        .await
        .err()
        .ok_or("error expected")?;
    assert_eq!(stale.code(), ErrorCode::Conflict);
    let original = library
        .sticker_detail(s.id())?
        .wait()
        .await?
        .original_path
        .ok_or("path expected")?;
    std::fs::remove_file(original)?;
    let missing = library
        .restore_sticker(s.id(), Generation::new(1)?, Revision::LOCAL)?
        .wait()
        .await
        .err()
        .ok_or("error expected")?;
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert!(
        !library
            .sticker_detail(s.id())?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .is_active()
    );
    library.close().await?;
    Ok(())
}
