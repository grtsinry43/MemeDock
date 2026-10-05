use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE export_artifacts (
            id TEXT PRIMARY KEY CHECK(length(id)=36),
            source_hash TEXT NOT NULL REFERENCES assets(hash),
            recipe TEXT NOT NULL CHECK(recipe='original-v1'),
            mime TEXT NOT NULL CHECK(mime IN ('image/png','image/jpeg','image/gif','image/webp')),
            byte_size INTEGER NOT NULL CHECK(byte_size>=0),
            animated INTEGER NOT NULL CHECK(animated IN (0,1)),
            retained_until INTEGER NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('ready','deleting')),
            CHECK(length(source_hash)=64)
        ) STRICT",
            )
            .await?;
        manager.get_connection().execute_unprepared("CREATE UNIQUE INDEX export_artifacts_ready ON export_artifacts(source_hash,recipe) WHERE state='ready'").await?;
        manager
            .get_connection()
            .execute_unprepared("PRAGMA user_version=2")
            .await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "destructive library downgrade is not supported".into(),
        ))
    }
}
