use sea_orm_migration::prelude::*;
mod m0001_initial;
mod m0002_export_artifacts;

pub(super) struct Migrator;
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m0001_initial::Migration),
            Box::new(m0002_export_artifacts::Migration),
        ]
    }
}
pub(super) const SCHEMA_VERSION: i64 = 2;
pub(super) const APPLICATION_ID: i64 = 0x4d444f43;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};

    #[tokio::test]
    async fn initial_migration_failure_rolls_back_all_business_tables()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut options = ConnectOptions::new("sqlite::memory:");
        options.max_connections(1).sqlx_logging(false);
        let db = Database::connect(options).await?;
        // A conflicting later CREATE TABLE forces failure after earlier DDL.
        db.execute_unprepared("CREATE TABLE tags(id TEXT)").await?;
        assert!(Migrator::up(&db, None).await.is_err());
        let row=db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT count(*) AS count FROM sqlite_schema WHERE name IN ('assets','stickers','collections','library_metadata')".to_owned())).await?.ok_or("schema count")?;
        assert_eq!(row.try_get::<i64>("", "count")?, 0);
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA user_version".to_owned(),
            ))
            .await?
            .ok_or("version")?;
        assert_eq!(row.try_get::<i64>("", "user_version")?, 0);
        db.close().await?;
        Ok(())
    }
}
