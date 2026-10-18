//! Initial server/sync compatibility baseline: MemeDock 0.2.0.
//! The source name identifies development on the 0.1.3 tree, not a released
//! 0.1.x sync format. Preserve the provisional authentication-only database as
//! a development upgrade path. All pre-release changes targeting 0.2.0 stay in
//! this module; caller owns rollback. Future released transitions start at 0.2.0.
use crate::Result;
use sea_orm::{ConnectionTrait, DbBackend, Statement};

pub(in crate::server) async fn apply(db: &impl ConnectionTrait) -> Result<()> {
    for sql in [
        "CREATE TABLE server_sync_library (singleton INTEGER PRIMARY KEY CHECK(singleton=1), library_id TEXT NOT NULL, epoch TEXT NOT NULL, head INTEGER NOT NULL CHECK(head>=0)) STRICT",
        "CREATE TABLE server_sync_entities (kind TEXT NOT NULL CHECK(kind IN ('sticker','collection','tag','collection_item','sticker_tag','tombstone')), id TEXT NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload)), sort_key TEXT COLLATE BINARY, parent_id TEXT, PRIMARY KEY(kind,id)) STRICT",
        "CREATE INDEX server_sync_order ON server_sync_entities(kind,parent_id,sort_key COLLATE BINARY,id)",
        "CREATE TABLE server_sync_events (seq INTEGER PRIMARY KEY CHECK(seq>0), payload TEXT NOT NULL CHECK(json_valid(payload))) STRICT",
        "CREATE TABLE server_sync_receipts (device_id TEXT NOT NULL REFERENCES server_devices(id), op_id TEXT NOT NULL, request TEXT NOT NULL CHECK(json_valid(request)), receipt TEXT NOT NULL CHECK(json_valid(receipt)), PRIMARY KEY(device_id,op_id)) STRICT",
        "CREATE TABLE server_assets (hash TEXT PRIMARY KEY CHECK(length(hash)=64), payload TEXT NOT NULL CHECK(json_valid(payload))) STRICT",
    ] {
        db.execute_unprepared(sql).await?;
    }
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO server_sync_library VALUES(1,?,?,0)",
        [
            memedock_domain::identity::LibraryId::new()
                .to_string()
                .into(),
            memedock_domain::identity::SyncEpoch::new()
                .to_string()
                .into(),
        ],
    ))
    .await?;
    db.execute_unprepared("PRAGMA user_version=2").await?;
    Ok(())
}

/// Observability added during 0.2.0 development. Historic receipt times are
/// unknown; never substitute a client clock or manufacture an acceptance time.
pub(in crate::server) async fn add_monitoring(db: &impl ConnectionTrait) -> Result<()> {
    for sql in [
        "ALTER TABLE server_sync_receipts ADD COLUMN received_at INTEGER",
        "CREATE INDEX server_receipt_time ON server_sync_receipts(received_at DESC,device_id,op_id)",
        "CREATE INDEX server_receipt_device_time ON server_sync_receipts(device_id,received_at DESC,op_id)",
        "CREATE TABLE server_device_progress (device_id TEXT PRIMARY KEY REFERENCES server_devices(id), epoch TEXT NOT NULL, applied_cursor INTEGER NOT NULL CHECK(applied_cursor>=0), observed_at INTEGER NOT NULL) STRICT",
        "PRAGMA user_version=3",
    ] {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

pub(in crate::server) async fn add_purge(db: &impl ConnectionTrait) -> Result<()> {
    db.execute_unprepared("CREATE TABLE server_purge_cleanup (hash TEXT PRIMARY KEY, generation INTEGER NOT NULL CHECK(generation>=0)) STRICT").await?;
    db.execute_unprepared("PRAGMA user_version=4").await?;
    Ok(())
}
