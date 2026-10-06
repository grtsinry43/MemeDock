use sea_orm_migration::prelude::*;
mod m0001_initial;
mod m0002_export_artifacts;
mod m0003_export_recipes;

pub(super) struct Migrator;
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m0001_initial::Migration),
            Box::new(m0002_export_artifacts::Migration),
            Box::new(m0003_export_recipes::Migration),
        ]
    }
}
pub(super) const SCHEMA_VERSION: i64 = 3;
pub(super) const APPLICATION_ID: i64 = 0x4d444f43;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};

    #[tokio::test]
    async fn version_two_outputs_upgrade_without_changing_identity_or_retention()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut options = ConnectOptions::new("sqlite::memory:");
        options.max_connections(1).sqlx_logging(false);
        let db = Database::connect(options).await?;
        Migrator::up(&db, Some(2)).await?;
        db.execute_unprepared("INSERT INTO assets VALUES(
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',5,'image/png',1,1,0,100);
            INSERT INTO export_artifacts VALUES(
            '019a0000-0000-7000-8000-000000000001',
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            'original-v1','image/png',5,0,123456,'ready');").await?;
        Migrator::up(&db, None).await?;
        let record = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT * FROM export_artifacts".to_owned(),
            ))
            .await?
            .ok_or("output missing")?;
        assert_eq!(
            record.try_get::<String>("", "id")?,
            "019a0000-0000-7000-8000-000000000001"
        );
        assert_eq!(
            record.try_get::<String>("", "source_hash")?,
            record.try_get::<String>("", "output_hash")?
        );
        assert_eq!(record.try_get::<i64>("", "retained_until")?, 123456);
        db.close().await?;
        Ok(())
    }

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
