use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite-specific constraints are deliberate. No automatic entity sync.
        for sql in STATEMENTS {
            manager.get_connection().execute_unprepared(sql).await?;
        }
        // Identity is created with the schema in the same migration transaction.
        // Never silently regenerate identity for an already initialized library.
        manager
            .get_connection()
            .execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
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
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "destructive library downgrade is not supported".into(),
        ))
    }
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
        PRIMARY KEY(collection_id,sticker_id)
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
    "CREATE INDEX stickers_recent ON stickers(created_at DESC,id DESC) WHERE deleted_at IS NULL",
    "CREATE INDEX stickers_deleted ON stickers(deleted_at DESC,id DESC) WHERE deleted_at IS NOT NULL",
    "CREATE INDEX collections_order ON collections(sort_key COLLATE BINARY,id) WHERE deleted_at IS NULL",
    "CREATE INDEX collection_items_order ON collection_items(collection_id,sort_key COLLATE BINARY,sticker_id) WHERE present=1",
    "CREATE INDEX collection_items_sticker ON collection_items(sticker_id)",
    "CREATE INDEX sticker_tags_tag ON sticker_tags(tag_id,sticker_id) WHERE present=1",
    "CREATE INDEX tags_name ON tags(normalized_name,id) WHERE deleted_at IS NULL",
    "CREATE INDEX local_changes_pending ON local_changes(status,local_order)",
    "PRAGMA application_id=1296322371",
    "PRAGMA user_version=1",
];
