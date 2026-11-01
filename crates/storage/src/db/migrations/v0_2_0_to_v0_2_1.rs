//! Preserve business rows and operation identity. The caller owns rollback.
use crate::Result;
use sea_orm::ConnectionTrait;

pub(in crate::db) async fn apply(db: &impl ConnectionTrait) -> Result<()> {
    // Usage and source provenance belong to the immutable content, and must
    // survive moving an incomplete sticker out of the published library.
    for sql in [
        "CREATE TABLE local_usage_v021 (sticker_id TEXT PRIMARY KEY REFERENCES assets(hash), last_used_at INTEGER NOT NULL, use_count INTEGER NOT NULL CHECK(use_count>=0)) STRICT",
        "INSERT INTO local_usage_v021 SELECT * FROM local_usage",
        "DROP TABLE local_usage",
        "ALTER TABLE local_usage_v021 RENAME TO local_usage",
        "CREATE TABLE telegram_items_v021 (unique_id TEXT PRIMARY KEY CHECK(length(unique_id) BETWEEN 1 AND 256), sticker_id TEXT NOT NULL REFERENCES assets(hash)) STRICT",
        "INSERT INTO telegram_items_v021 SELECT * FROM telegram_items",
        "DROP TABLE telegram_items",
        "ALTER TABLE telegram_items_v021 RENAME TO telegram_items",
    ] {
        db.execute_unprepared(sql).await?;
    }
    db.execute_unprepared("CREATE TABLE client_sync_transfers (hash TEXT PRIMARY KEY REFERENCES assets(hash), next_retry_ms INTEGER NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload))) STRICT").await?;
    db.execute_unprepared(
        "UPDATE client_sync_jobs SET payload=json_set(payload,'$.envelope.protocol_version',2)",
    )
    .await?;
    db.execute_unprepared("PRAGMA user_version=7").await?;
    Ok(())
}
