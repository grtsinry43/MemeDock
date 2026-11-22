#[allow(dead_code)]
mod common;
use common::*;
use memedock_domain::{asset::ImageFormat, identity::OperationId};
use memedock_storage::{LibraryDatabase, artifacts::ArtifactRecord};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
async fn released_v025_outputs_and_clipboard_survive_atomic_upgrade_and_retry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    let db = LibraryDatabase::open(&path).await?;
    let identity = db.identity();
    let (asset, sticker) = fixture(b"old original", "preserved", 100)?;
    insert(&db, &asset, &sticker).await?;
    let record = ArtifactRecord {
        id: OperationId::new(),
        source_hash: asset.hash(),
        source_asset_hash: Some(asset.hash()),
        output_hash: asset.hash(),
        recipe: "original-v1".into(),
        format: ImageFormat::Png,
        byte_size: 12,
        animated: false,
        retained_until: 123456,
        deleting: false,
    };
    db.save_artifact(&record).await?;
    let pin = OperationId::new();
    db.protect_clipboard(pin, record.id).await?;
    db.close().await?;
    let file = path.clone();
    let mut options = ConnectOptions::new("sqlite://migration.sqlite");
    options
        .max_connections(1)
        .sqlx_logging(false)
        .map_sqlx_sqlite_opts(move |o| o.filename(&file).foreign_keys(true));
    let raw = Database::connect(options.clone()).await?;
    // Restore the actual released parent FK rather than merely changing the
    // schema marker on a current table. Children remain protected throughout.
    for sql in [
        "CREATE TABLE pins_backup AS SELECT * FROM clipboard_references",
        "DROP TABLE clipboard_references",
        "CREATE TABLE old_export_artifacts (id TEXT PRIMARY KEY,source_hash TEXT NOT NULL REFERENCES assets(hash),output_hash TEXT NOT NULL,recipe TEXT NOT NULL,mime TEXT NOT NULL,byte_size INTEGER NOT NULL,animated INTEGER NOT NULL,retained_until INTEGER NOT NULL,state TEXT NOT NULL) STRICT",
        "INSERT INTO old_export_artifacts SELECT id,source_hash,output_hash,recipe,mime,byte_size,animated,retained_until,state FROM export_artifacts",
        "DROP TABLE export_artifacts",
        "ALTER TABLE old_export_artifacts RENAME TO export_artifacts",
        "CREATE UNIQUE INDEX export_artifacts_ready ON export_artifacts(source_hash,recipe) WHERE state='ready'",
        "CREATE TABLE clipboard_references (id TEXT PRIMARY KEY,artifact_id TEXT NOT NULL REFERENCES export_artifacts(id) ON DELETE RESTRICT,state TEXT NOT NULL) STRICT",
        "INSERT INTO clipboard_references SELECT * FROM pins_backup",
        "DROP TABLE pins_backup",
        "PRAGMA user_version=7",
        "CREATE TABLE export_artifacts_v030(collision INTEGER)",
    ] {
        raw.execute_unprepared(sql).await?;
    }
    raw.close().await?;
    assert!(LibraryDatabase::open(&path).await.is_err());
    let raw = Database::connect(options.clone()).await?;
    let marker = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA user_version".to_owned(),
        ))
        .await?
        .ok_or("marker")?;
    assert_eq!(marker.try_get::<i64>("", "user_version")?, 7);
    let pins = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT count(*) AS n FROM clipboard_references".to_owned(),
        ))
        .await?
        .ok_or("pins")?;
    assert_eq!(pins.try_get::<i64>("", "n")?, 1);
    let partial = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT count(*) AS n FROM sqlite_schema WHERE name='creator_clipboard_backup'"
                .to_owned(),
        ))
        .await?
        .ok_or("partial")?;
    assert_eq!(partial.try_get::<i64>("", "n")?, 0);
    raw.execute_unprepared("DROP TABLE export_artifacts_v030")
        .await?;
    raw.close().await?;
    let db = LibraryDatabase::open(&path).await?;
    assert_eq!(db.identity(), identity);
    assert_eq!(db.sticker(sticker.id()).await?, Some(sticker));
    let restored = db
        .artifact_by_id(record.id)
        .await?
        .ok_or("output missing")?;
    assert_eq!(restored.source_asset_hash, Some(asset.hash()));
    assert_eq!(restored.retained_until, 123456);
    assert!(db.artifact_clipboard_protected(record.id).await?);
    assert!(db.remove_artifact(record.id).await.is_err());
    let (external, _) = fixture(b"not in library", "creation", 100)?;
    let standalone = ArtifactRecord {
        id: OperationId::new(),
        source_hash: external.hash(),
        source_asset_hash: None,
        output_hash: external.hash(),
        recipe: "creator-webp-v1".into(),
        format: ImageFormat::WebP,
        byte_size: 14,
        animated: true,
        retained_until: 999,
        deleting: false,
    };
    db.save_artifact(&standalone).await?;
    assert!(db.asset(external.hash()).await?.is_none());
    db.close().await?;
    let reopened = LibraryDatabase::open(&path).await?;
    assert!(reopened.artifact_by_id(standalone.id).await?.is_some());
    assert!(reopened.artifact_clipboard_protected(record.id).await?);
    reopened.close().await?;
    let raw = Database::connect(options).await?;
    assert!(
        raw.query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check".to_owned()
        ))
        .await?
        .is_empty()
    );
    raw.close().await?;
    Ok(())
}
