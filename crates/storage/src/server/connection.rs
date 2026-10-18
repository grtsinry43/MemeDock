use crate::{Result, StorageError};
use sea_orm::sqlx::sqlite::{SqliteJournalMode, SqliteSynchronous};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    SqliteTransactionMode, Statement, TransactionOptions, TransactionTrait,
};
use std::{path::Path, time::Duration};

#[derive(Clone, Debug)]
pub struct ServerDatabase {
    pub(super) connection: DatabaseConnection,
}

impl ServerDatabase {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or(
            StorageError::InvalidInput("database needs a parent directory"),
        )?;
        std::fs::create_dir_all(parent)?;
        let path = std::fs::canonicalize(parent)?.join(
            path.file_name()
                .ok_or(StorageError::InvalidInput("database filename"))?,
        );
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(StorageError::InvalidInput("database symlink"));
        }
        let mut options = ConnectOptions::new("sqlite://server.sqlite");
        options
            .min_connections(1)
            .max_connections(4)
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(move |o| {
                o.filename(&path)
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .foreign_keys(true)
                    .synchronous(SqliteSynchronous::Full)
                    .busy_timeout(Duration::from_secs(5))
            });
        let connection = Database::connect(options).await?;
        let tx = connection
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await?;
        let application = pragma(&tx, "application_id").await?;
        let version = pragma(&tx, "user_version").await?;
        if application == 0 && version == 0 {
            let row = tx.query_one_raw(Statement::from_string(DbBackend::Sqlite,
                "SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'".to_owned())).await?
                .ok_or(StorageError::Integrity("schema count missing"))?;
            if row.try_get::<i64>("", "count")? != 0 {
                return Err(StorageError::UnsupportedSchema);
            }
            super::schema::initialize(&tx).await?;
            super::migrations::v0_1_3_to_v0_2_0::apply(&tx).await?;
            super::migrations::v0_1_3_to_v0_2_0::add_monitoring(&tx).await?;
        } else if application != super::schema::APPLICATION_ID || !(1..=4).contains(&version) {
            return Err(StorageError::UnsupportedSchema);
        } else if version == 1 {
            super::migrations::v0_1_3_to_v0_2_0::apply(&tx).await?;
            super::migrations::v0_1_3_to_v0_2_0::add_monitoring(&tx).await?;
        } else if version == 2 {
            super::migrations::v0_1_3_to_v0_2_0::add_monitoring(&tx).await?;
        }
        if version < 4 {
            super::migrations::v0_1_3_to_v0_2_0::add_purge(&tx).await?;
        }
        tx.commit().await?;
        Ok(Self { connection })
    }
    pub async fn close(self) -> Result<()> {
        self.connection.close().await?;
        Ok(())
    }
}

async fn pragma(db: &impl ConnectionTrait, name: &str) -> Result<i64> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA {name}"),
        ))
        .await?
        .ok_or(StorageError::Integrity("pragma missing"))?;
    Ok(row.try_get("", name)?)
}
