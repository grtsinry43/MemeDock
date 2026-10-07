mod common;
use common::*;
use memedock_core::{
    ErrorCode, Library,
    batch::{BatchAction, BatchOutcome, BatchTarget},
};
use memedock_domain::{identity::StickerId, tag::Name, version::Generation};

#[tokio::test]
async fn single_assignment_and_tag_deltas_are_atomic_and_survive_restart() -> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, sticker) = seed(&config, b"batch organization fixture").await?;
    let library = Library::open(config.clone()).await?;
    let a = library
        .create_collection(Name::new("A".into())?, None)?
        .wait()
        .await?;
    let b = library
        .create_collection(Name::new("B".into())?, None)?
        .wait()
        .await?;
    let tag = library
        .create_tag(Name::new("First".into())?)?
        .wait()
        .await?;
    let extra = library
        .create_tag(Name::new("Second".into())?)?
        .wait()
        .await?;
    let target = BatchTarget {
        id: sticker.id(),
        generation: Generation::INITIAL,
        deleted_revision: None,
    };
    library
        .set_sticker_organization(
            sticker.id(),
            Generation::INITIAL,
            Some(Some((a.id(), Generation::INITIAL))),
            Some(vec![(tag.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    let report = library
        .batch(
            vec![target, target],
            BatchAction::Assign(b.id(), Generation::INITIAL),
        )?
        .wait()
        .await?;
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].outcome, BatchOutcome::Applied);
    let repeated = library
        .batch(
            vec![target],
            BatchAction::Assign(b.id(), Generation::INITIAL),
        )?
        .wait()
        .await?;
    assert_eq!(repeated.items[0].outcome, BatchOutcome::Unchanged);
    // Invalid tag generation must roll back the collection move in the same transaction.
    let error = library
        .set_sticker_organization(
            sticker.id(),
            Generation::INITIAL,
            Some(Some((a.id(), Generation::INITIAL))),
            Some(vec![(tag.id(), Generation::new(1)?)]),
        )?
        .wait()
        .await
        .err()
        .ok_or("failure expected")?;
    assert_eq!(error.code(), ErrorCode::Conflict);
    let stale_clear = library
        .batch(
            vec![target],
            BatchAction::ClearCollection(Some((a.id(), Generation::INITIAL))),
        )?
        .wait()
        .await?;
    assert_eq!(
        stale_clear.items[0].outcome,
        BatchOutcome::Failed(ErrorCode::Conflict)
    );
    let absent = BatchTarget {
        id: StickerId::new(memedock_domain::identity::ContentHash::from_bytes([7; 32])),
        ..target
    };
    let added = library
        .batch(
            vec![target, absent],
            BatchAction::AddTags(vec![(extra.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    assert_eq!(added.items[0].outcome, BatchOutcome::Applied);
    assert_eq!(
        added.items[1].outcome,
        BatchOutcome::Failed(ErrorCode::NotFound)
    );
    let repeated = library
        .batch(
            vec![target],
            BatchAction::AddTags(vec![(extra.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    assert_eq!(repeated.items[0].outcome, BatchOutcome::Unchanged);
    assert_eq!(
        library
            .sticker_detail(sticker.id())?
            .wait()
            .await?
            .tags
            .len(),
        2
    );
    library
        .batch(
            vec![target],
            BatchAction::RemoveTags(vec![(extra.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    let summaries = library.collection_summaries()?.wait().await?;
    assert_eq!(
        summaries
            .iter()
            .find(|s| s.collection.id() == a.id())
            .ok_or("A")?
            .count,
        0
    );
    let summary = summaries
        .iter()
        .find(|s| s.collection.id() == b.id())
        .ok_or("B")?;
    assert_eq!(summary.count, 1);
    assert_eq!(summary.cover, Some(sticker.id()));
    library.close().await?;
    let reopened = Library::open(config).await?;
    let detail = reopened.sticker_detail(sticker.id())?.wait().await?;
    assert_eq!(detail.collection.ok_or("collection")?.id(), b.id());
    assert_eq!(
        detail.tags.iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![tag.id()]
    );
    reopened.close().await?;
    Ok(())
}
