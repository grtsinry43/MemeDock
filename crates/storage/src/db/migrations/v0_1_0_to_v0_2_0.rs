//! All released 0.1.x libraries use structure 4. Sync first ships in 0.2.0.
//! Caller owns the transaction; originals and existing business rows are retained.
use crate::Result;
use sea_orm::ConnectionTrait;

pub(in crate::db) async fn apply(db: &impl ConnectionTrait) -> Result<()> {
    db.execute_unprepared("CREATE TABLE client_sync (singleton INTEGER PRIMARY KEY CHECK(singleton=1), payload TEXT NOT NULL CHECK(json_valid(payload))) STRICT").await?;
    db.execute_unprepared("CREATE TABLE client_sync_jobs (ordinal INTEGER PRIMARY KEY AUTOINCREMENT, source_order INTEGER UNIQUE, payload TEXT NOT NULL CHECK(json_valid(payload))) STRICT").await?;
    add_purge(db).await?;
    Ok(())
}

pub(in crate::db) async fn add_purge(db: &impl ConnectionTrait) -> Result<()> {
    for table in ["stickers", "collections", "tags", "entity_tombstones"] {
        db.execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN purged_at INTEGER CHECK(purged_at IS NULL OR deleted_at IS NOT NULL)")).await?;
    }
    db.execute_unprepared("CREATE TABLE purge_cleanup (hash TEXT PRIMARY KEY REFERENCES assets(hash), generation INTEGER NOT NULL CHECK(generation>=0)) STRICT").await?;
    db.execute_unprepared("PRAGMA user_version=6").await?;
    Ok(())
}
