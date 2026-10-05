mod common;
use common::*;
use memedock_core::Library;
use memedock_domain::{tag::Name, version::Generation};

#[tokio::test]
async fn concurrent_normalized_names_reuse_one_active_tag_and_deleted_names_stay_deleted()
-> TestResult {
    let directory = tempfile::tempdir()?;
    let config = config(directory.path());
    let (_, sticker) = seed(&config, b"tag search original").await?;
    let library = Library::open(config).await?;
    let a = library.create_tag(Name::new(" ＣＡＴ ".into())?)?;
    let b = library.create_tag(Name::new("cat".into())?)?;
    let a = a.wait().await?;
    let b = b.wait().await?;
    assert_eq!(a.id(), b.id());
    library
        .set_sticker_relations(
            sticker.id(),
            Generation::INITIAL,
            None,
            Some(vec![(a.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    library
        .rename_tag(a.id(), Generation::INITIAL, Name::new("猫猫".into())?)?
        .wait()
        .await?;
    library
        .delete_tag(a.id(), Generation::INITIAL)?
        .wait()
        .await?;
    let replacement = library
        .create_tag(Name::new("猫猫".into())?)?
        .wait()
        .await?;
    assert_ne!(replacement.id(), a.id());
    assert_eq!(library.tags(true)?.wait().await?.len(), 1);
    assert_eq!(library.tags(false)?.wait().await?.len(), 1);
    library.close().await?;
    Ok(())
}
