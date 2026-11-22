//! 0.2.1 through 0.2.5 share structure 7. Creator outputs gain a cache identity
//! independent of the optional library asset relation. The caller owns the
//! immediate transaction: every table copy, reference and marker rolls back.
use crate::{Result, StorageError};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

pub(in crate::db) async fn apply(db: &impl ConnectionTrait) -> Result<()> {
    // Move the child rows out before rebuilding their parent; foreign key
    // enforcement remains enabled throughout the transaction.
    for sql in [
        "CREATE TABLE creator_clipboard_backup AS SELECT id,artifact_id,state FROM clipboard_references",
        "DROP TABLE clipboard_references",
        "CREATE TABLE export_artifacts_v030 (
            id TEXT PRIMARY KEY CHECK(length(id)=36),
            source_hash TEXT NOT NULL CHECK(length(source_hash)=64 AND source_hash NOT GLOB '*[^0-9a-f]*'),
            source_asset_hash TEXT REFERENCES assets(hash) CHECK(source_asset_hash IS NULL OR source_asset_hash=source_hash),
            output_hash TEXT NOT NULL CHECK(length(output_hash)=64),
            recipe TEXT NOT NULL CHECK(length(recipe) BETWEEN 1 AND 256),
            mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/gif','image/webp')),
            byte_size INTEGER NOT NULL CHECK(byte_size>=0),animated INTEGER NOT NULL CHECK(animated IN (0,1)),
            retained_until INTEGER NOT NULL,state TEXT NOT NULL CHECK(state IN ('ready','deleting'))
        ) STRICT",
        "INSERT INTO export_artifacts_v030(id,source_hash,source_asset_hash,output_hash,recipe,mime,byte_size,animated,retained_until,state)
            SELECT id,source_hash,source_hash,output_hash,recipe,mime,byte_size,animated,retained_until,state FROM export_artifacts",
        "DROP TABLE export_artifacts",
        "ALTER TABLE export_artifacts_v030 RENAME TO export_artifacts",
        "CREATE UNIQUE INDEX export_artifacts_ready ON export_artifacts(source_hash,recipe) WHERE state='ready'",
        "CREATE TABLE clipboard_references (
            id TEXT PRIMARY KEY CHECK(length(id)=36),
            artifact_id TEXT NOT NULL REFERENCES export_artifacts(id) ON DELETE RESTRICT,
            state TEXT NOT NULL CHECK(state IN ('pending','current'))
        ) STRICT",
        "INSERT INTO clipboard_references SELECT id,artifact_id,state FROM creator_clipboard_backup",
        "DROP TABLE creator_clipboard_backup",
    ] { db.execute_unprepared(sql).await?; }
    if !db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA foreign_key_check".to_owned(),
        ))
        .await?
        .is_empty()
    {
        return Err(StorageError::Integrity(
            "Creator migration foreign key check failed",
        ));
    }
    db.execute_unprepared("PRAGMA user_version=8").await?;
    Ok(())
}
