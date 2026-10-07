use crate::Result;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};

// An exact identifier for the current structure, never an upgrade target.
pub(super) const SCHEMA_VERSION: i64 = 4;
pub(super) const APPLICATION_ID: i64 = 0x4d444f43;

pub(super) async fn initialize(db: &DatabaseConnection) -> Result<()> {
    let tx = db.begin().await?;
    for sql in STATEMENTS {
        tx.execute_unprepared(sql).await?;
    }
    tx.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO library_metadata(singleton,library_id,device_id) VALUES(1,?,?)",
        [
            memedock_domain::identity::LibraryId::new()
                .to_string()
                .into(),
            memedock_domain::identity::DeviceId::new()
                .to_string()
                .into(),
        ],
    ))
    .await?;
    tx.commit().await?;
    Ok(())
}

const STATEMENTS: &[&str] = &[
    "CREATE TABLE assets (
        hash TEXT PRIMARY KEY CHECK(length(hash)=64 AND hash NOT GLOB '*[^0-9a-f]*'),
        byte_size INTEGER NOT NULL CHECK(byte_size>=0),
        mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/gif','image/webp')),
        width INTEGER NOT NULL CHECK(width BETWEEN 1 AND 4294967295),
        height INTEGER NOT NULL CHECK(height BETWEEN 1 AND 4294967295),
        animated INTEGER NOT NULL CHECK(animated IN (0,1)), created_at INTEGER NOT NULL
    ) STRICT",
    "CREATE TABLE stickers (
        id TEXT PRIMARY KEY REFERENCES assets(hash), title TEXT NOT NULL,
        original_name TEXT NOT NULL, note TEXT NOT NULL,
        starred INTEGER NOT NULL CHECK(starred IN (0,1)),
        generation INTEGER NOT NULL CHECK(generation>=0), revision INTEGER NOT NULL CHECK(revision>=0),
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER,
        normalized_search_text TEXT NOT NULL
    ) STRICT",
    "CREATE TABLE collections (
        id TEXT PRIMARY KEY CHECK(length(id)=36), name TEXT NOT NULL CHECK(length(trim(name))>0),
        sort_key TEXT COLLATE BINARY NOT NULL CHECK(length(sort_key)>0 AND length(sort_key)%2=0 AND sort_key NOT GLOB '*[^0-9a-f]*'),
        generation INTEGER NOT NULL CHECK(generation>=0), revision INTEGER NOT NULL CHECK(revision>=0),
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER
    ) STRICT",
    "CREATE TABLE tags (
        id TEXT PRIMARY KEY CHECK(length(id)=36), name TEXT NOT NULL CHECK(length(trim(name))>0),
        normalized_name TEXT NOT NULL CHECK(length(normalized_name)>0),
        generation INTEGER NOT NULL CHECK(generation>=0), revision INTEGER NOT NULL CHECK(revision>=0),
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER
    ) STRICT",
    "CREATE TABLE collection_items (
        collection_id TEXT NOT NULL REFERENCES collections(id), sticker_id TEXT NOT NULL REFERENCES stickers(id),
        collection_generation INTEGER NOT NULL CHECK(collection_generation>=0),
        sticker_generation INTEGER NOT NULL CHECK(sticker_generation>=0),
        present INTEGER NOT NULL CHECK(present IN (0,1)),
        sort_key TEXT COLLATE BINARY NOT NULL CHECK(length(sort_key)>0 AND length(sort_key)%2=0 AND sort_key NOT GLOB '*[^0-9a-f]*'),
        revision INTEGER NOT NULL CHECK(revision>=0), updated_at INTEGER NOT NULL,
        PRIMARY KEY(sticker_id)
    ) STRICT",
    "CREATE TABLE sticker_tags (
        sticker_id TEXT NOT NULL REFERENCES stickers(id), tag_id TEXT NOT NULL REFERENCES tags(id),
        sticker_generation INTEGER NOT NULL CHECK(sticker_generation>=0), tag_generation INTEGER NOT NULL CHECK(tag_generation>=0),
        present INTEGER NOT NULL CHECK(present IN (0,1)), revision INTEGER NOT NULL CHECK(revision>=0),
        updated_at INTEGER NOT NULL, PRIMARY KEY(sticker_id,tag_id)
    ) STRICT",
    "CREATE TABLE entity_tombstones (
        entity_kind TEXT NOT NULL CHECK(entity_kind IN ('sticker','collection','tag')),
        entity_id TEXT NOT NULL CHECK(
            (entity_kind='sticker' AND length(entity_id)=64 AND entity_id NOT GLOB '*[^0-9a-f]*') OR
            (entity_kind IN ('collection','tag') AND length(entity_id)=36)),
        generation INTEGER NOT NULL CHECK(generation>=0), revision INTEGER NOT NULL CHECK(revision>=0),
        deleted_at INTEGER NOT NULL, PRIMARY KEY(entity_kind,entity_id)
    ) STRICT",
    "CREATE TABLE local_assets (
        hash TEXT PRIMARY KEY REFERENCES assets(hash),
        blob_status TEXT NOT NULL CHECK(blob_status IN ('missing','downloading','ready','failed')),
        thumb_status TEXT NOT NULL CHECK(thumb_status IN ('missing','generating','ready','failed')),
        last_error TEXT, verified_at INTEGER
    ) STRICT",
    "CREATE TABLE local_usage (
        sticker_id TEXT PRIMARY KEY REFERENCES stickers(id), last_used_at INTEGER NOT NULL,
        use_count INTEGER NOT NULL CHECK(use_count>=0)
    ) STRICT",
    "CREATE TABLE local_changes (
        local_order INTEGER PRIMARY KEY AUTOINCREMENT CHECK(local_order>0), op_id TEXT UNIQUE NOT NULL CHECK(length(op_id)=36),
        schema_version INTEGER NOT NULL CHECK(schema_version=1),
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        status TEXT NOT NULL CHECK(status IN ('pending','sending','accepted_waiting_pull','confirmed','blocked','sealed')),
        attempts INTEGER NOT NULL CHECK(attempts BETWEEN 0 AND 4294967295), retry_after INTEGER,
        accepted_revision INTEGER CHECK(accepted_revision>=0), last_error TEXT, created_at INTEGER NOT NULL,
        CHECK(status NOT IN ('sending','accepted_waiting_pull','confirmed') OR attempts>0),
        CHECK(status!='sealed' OR attempts=0),
        CHECK((status='accepted_waiting_pull' AND accepted_revision IS NOT NULL) OR status!='accepted_waiting_pull'),
        CHECK(accepted_revision IS NULL OR status IN ('accepted_waiting_pull','confirmed','blocked')),
        CHECK(retry_after IS NULL OR status='pending')
    ) STRICT",
    "CREATE TABLE library_metadata (
        singleton INTEGER PRIMARY KEY CHECK(singleton=1),
        library_id TEXT NOT NULL CHECK(length(library_id)=36), device_id TEXT NOT NULL CHECK(length(device_id)=36)
    ) STRICT",
    "CREATE TABLE export_artifacts (
        id TEXT PRIMARY KEY CHECK(length(id)=36),
        source_hash TEXT NOT NULL REFERENCES assets(hash) CHECK(length(source_hash)=64),
        output_hash TEXT NOT NULL CHECK(length(output_hash)=64),
        recipe TEXT NOT NULL CHECK(length(recipe) BETWEEN 1 AND 256),
        mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/gif','image/webp')),
        byte_size INTEGER NOT NULL CHECK(byte_size>=0), animated INTEGER NOT NULL CHECK(animated IN (0,1)),
        retained_until INTEGER NOT NULL, state TEXT NOT NULL CHECK(state IN ('ready','deleting'))
    ) STRICT",
    "CREATE UNIQUE INDEX export_artifacts_ready ON export_artifacts(source_hash,recipe) WHERE state='ready'",
    "CREATE TABLE clipboard_references (
        id TEXT PRIMARY KEY CHECK(length(id)=36),
        artifact_id TEXT NOT NULL REFERENCES export_artifacts(id) ON DELETE RESTRICT,
        state TEXT NOT NULL CHECK(state IN ('pending','current'))
    ) STRICT",
    "CREATE UNIQUE INDEX clipboard_current ON clipboard_references(state) WHERE state='current'",
    "CREATE INDEX clipboard_artifact ON clipboard_references(artifact_id)",
    "CREATE INDEX stickers_recent ON stickers(created_at DESC,id DESC) WHERE deleted_at IS NULL",
    "CREATE INDEX stickers_deleted ON stickers(deleted_at DESC,id DESC) WHERE deleted_at IS NOT NULL",
    "CREATE INDEX collections_order ON collections(sort_key COLLATE BINARY,id) WHERE deleted_at IS NULL",
    "CREATE INDEX collection_items_order ON collection_items(collection_id,sort_key COLLATE BINARY,sticker_id) WHERE present=1",
    "CREATE INDEX collection_items_sticker ON collection_items(sticker_id)",
    "CREATE INDEX sticker_tags_tag ON sticker_tags(tag_id,sticker_id) WHERE present=1",
    "CREATE INDEX tags_name ON tags(normalized_name,id) WHERE deleted_at IS NULL",
    "CREATE INDEX local_changes_pending ON local_changes(status,local_order)",
    "PRAGMA application_id=1296322371",
    "PRAGMA user_version=4",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn failed_initialization_rolls_back_all_created_tables()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let db = sea_orm::Database::connect("sqlite::memory:").await?;
        db.execute_unprepared("CREATE TABLE tags (collision INTEGER)")
            .await?;
        assert!(initialize(&db).await.is_err());
        let row=db.query_one_raw(Statement::from_string(DbBackend::Sqlite,"SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'".to_owned())).await?.ok_or("table count missing")?;
        assert_eq!(row.try_get::<i64>("", "count")?, 1);
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA user_version".to_owned(),
            ))
            .await?
            .ok_or("schema marker missing")?;
        assert_eq!(row.try_get::<i64>("", "user_version")?, 0);
        db.close().await?;
        Ok(())
    }
}
