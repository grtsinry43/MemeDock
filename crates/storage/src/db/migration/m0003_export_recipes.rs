use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rebuild rather than mutating m0002: existing URI identities survive.
        manager.get_connection().execute_unprepared(
            "CREATE TABLE export_artifacts_v3 (
                id TEXT PRIMARY KEY CHECK(length(id)=36),
                source_hash TEXT NOT NULL REFERENCES assets(hash) CHECK(length(source_hash)=64),
                output_hash TEXT NOT NULL CHECK(length(output_hash)=64),
                recipe TEXT NOT NULL CHECK(length(recipe) BETWEEN 1 AND 256),
                mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/gif','image/webp')),
                byte_size INTEGER NOT NULL CHECK(byte_size>=0),
                animated INTEGER NOT NULL CHECK(animated IN (0,1)),
                retained_until INTEGER NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('ready','deleting'))
            ) STRICT;
            INSERT INTO export_artifacts_v3
                SELECT id,source_hash,source_hash,recipe,mime,byte_size,animated,retained_until,state
                FROM export_artifacts;
            DROP TABLE export_artifacts;
            ALTER TABLE export_artifacts_v3 RENAME TO export_artifacts;
            CREATE UNIQUE INDEX export_artifacts_ready ON export_artifacts(source_hash,recipe) WHERE state='ready';
            CREATE TABLE clipboard_references (
                id TEXT PRIMARY KEY CHECK(length(id)=36),
                artifact_id TEXT NOT NULL REFERENCES export_artifacts(id) ON DELETE RESTRICT,
                state TEXT NOT NULL CHECK(state IN ('pending','current'))
            ) STRICT;
            CREATE UNIQUE INDEX clipboard_current ON clipboard_references(state) WHERE state='current';
            CREATE INDEX clipboard_artifact ON clipboard_references(artifact_id);
            PRAGMA user_version=3;"
        ).await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "destructive library downgrade is not supported".into(),
        ))
    }
}
