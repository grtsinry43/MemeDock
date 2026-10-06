mod common;
use common::*;
use memedock_domain::{
    change::{Operation, OperationKind},
    identity::OperationId,
    identity::TagId,
    tag::{Name, Tag},
    version::TimestampMs,
};
use memedock_storage::{LibraryDatabase, StorageError};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};

async fn raw(path: &std::path::Path) -> TestResult<DatabaseConnection> {
    let path = path.to_owned();
    let mut options = ConnectOptions::new("sqlite://library.sqlite");
    options
        .max_connections(1)
        .sqlx_logging(false)
        .map_sqlx_sqlite_opts(move |o| {
            o.filename(&path).create_if_missing(true).foreign_keys(true)
        });
    Ok(Database::connect(options).await?)
}

#[tokio::test]
async fn restart_preserves_identity_models_and_log() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("库 ?#%.sqlite");
    let db = LibraryDatabase::open(&path).await?;
    let identity = db.identity();
    let (asset, sticker) = fixture(b"persist", "猫猫", 123)?;
    insert(&db, &asset, &sticker).await?;
    db.close().await?;
    let db = LibraryDatabase::open(&path).await?;
    assert_eq!(db.identity(), identity);
    assert_eq!(db.asset(asset.hash()).await?, Some(asset));
    assert_eq!(db.sticker(sticker.id()).await?, Some(sticker));
    assert_eq!(db.changes_after(None, 10).await?.len(), 1);
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn rejects_non_current_schema_and_unrelated_database() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    LibraryDatabase::open(&path).await?.close().await?;
    for version in [3, 999] {
        let conn = raw(&path).await?;
        conn.execute_unprepared(&format!("PRAGMA user_version={version}"))
            .await?;
        conn.close().await?;
        assert!(matches!(
            LibraryDatabase::open(&path).await,
            Err(StorageError::UnsupportedSchema)
        ));
        let conn = raw(&path).await?;
        let marker = conn
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "PRAGMA user_version".to_owned(),
            ))
            .await?
            .ok_or("marker missing")?;
        assert_eq!(
            marker.try_get::<i64>("", "user_version")?,
            version,
            "opening must not convert the existing database"
        );
        conn.close().await?;
    }
    let other = dir.path().join("other.sqlite");
    let conn = raw(&other).await?;
    conn.execute_unprepared("CREATE TABLE unrelated (id INTEGER)")
        .await?;
    conn.close().await?;
    assert!(matches!(
        LibraryDatabase::open(&other).await,
        Err(StorageError::UnsupportedSchema)
    ));
    Ok(())
}
#[tokio::test]
async fn sqlite_constraints_and_domain_mapping_reject_invalid_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, sticker) = fixture(b"constraints", "猫", 100)?;
    insert(&db, &asset, &sticker).await?;
    let tag = Tag::new(
        TagId::new(),
        Name::new("猫咪".into())?,
        TimestampMs::new(100),
    );
    let mut tx = db.begin_write().await?;
    tx.save_tag(&tag).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::CreateTag {
            tag_id: tag.id(),
            name: tag.name().clone(),
        })?,
        TimestampMs::new(100),
    )
    .await?;
    tx.commit().await?;
    let conn = raw(db.path()).await?;
    assert!(
        conn.execute_unprepared("UPDATE stickers SET generation=-1")
            .await
            .is_err()
    );
    assert!(conn.execute_unprepared("UPDATE stickers SET id='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'").await.is_err());
    assert!(
        conn.execute_unprepared("UPDATE assets SET width=0")
            .await
            .is_err()
    );
    conn.execute_unprepared("UPDATE tags SET normalized_name='wrong'")
        .await?;
    assert!(matches!(
        db.tag(tag.id()).await,
        Err(StorageError::InvalidData { table: "tags", .. })
    ));
    conn.close().await?;
    db.close().await?;
    Ok(())
}
#[tokio::test]
async fn snapshot_contains_committed_wal_and_never_overwrites() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
    let (asset, sticker) = fixture(b"snapshot", "截图", 100)?;
    insert(&db, &asset, &sticker).await?;
    let destination = dir.path().join("backup.sqlite");
    db.snapshot(&destination).await?;
    assert!(matches!(
        db.snapshot(&destination).await,
        Err(StorageError::Conflict(_))
    ));
    let snapshot = LibraryDatabase::open(&destination).await?;
    assert_eq!(snapshot.sticker(sticker.id()).await?, Some(sticker));
    assert_eq!(snapshot.identity(), db.identity());
    let raw = raw(db.path()).await?;
    let row = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA integrity_check".to_owned(),
        ))
        .await?
        .ok_or("integrity result")?;
    assert_eq!(row.try_get::<String>("", "integrity_check")?, "ok");
    raw.close().await?;
    snapshot.close().await?;
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn lost_identity_is_an_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    let db = LibraryDatabase::open(&path).await?;
    let conn = raw(&path).await?;
    conn.execute_unprepared("DELETE FROM library_metadata")
        .await?;
    conn.close().await?;
    db.close().await?;
    assert!(matches!(
        LibraryDatabase::open(&path).await,
        Err(StorageError::Integrity("library identity is missing"))
    ));
    Ok(())
}
