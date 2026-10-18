use crate::Result;
use sea_orm::ConnectionTrait;

pub(super) const APPLICATION_ID: i64 = 0x4d445352;

pub(super) async fn initialize(db: &impl ConnectionTrait) -> Result<()> {
    for sql in [
        "CREATE TABLE server_admin (singleton INTEGER PRIMARY KEY CHECK(singleton=1), username TEXT NOT NULL, password_hash TEXT NOT NULL, auth_version TEXT NOT NULL) STRICT",
        "CREATE TABLE server_sessions (id_hash TEXT PRIMARY KEY, payload TEXT NOT NULL CHECK(json_valid(payload)), expires_at INTEGER NOT NULL, auth_version TEXT NOT NULL) STRICT",
        "CREATE INDEX server_sessions_expiry ON server_sessions(expires_at)",
        "CREATE TABLE server_devices (id TEXT PRIMARY KEY CHECK(length(id)=36), name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80), token_hash TEXT NOT NULL CHECK(length(token_hash)=64), created_at INTEGER NOT NULL, revoked_at INTEGER, last_seen_at INTEGER) STRICT",
    ] {
        db.execute_unprepared(sql).await?;
    }
    db.execute_unprepared(&format!("PRAGMA application_id={APPLICATION_ID}"))
        .await?;
    db.execute_unprepared("PRAGMA user_version=1").await?;
    Ok(())
}
