use super::schema::{self, APPLICATION_ID, SCHEMA_VERSION};
use crate::{Result, StorageError, entities::library_metadata};
use memedock_domain::identity::{DeviceId, LibraryId};
use sea_orm::sqlx::sqlite::{SqliteJournalMode, SqliteSynchronous};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    Statement,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibraryIdentity {
    pub library_id: LibraryId,
    pub device_id: DeviceId,
}

/// One pool per library. The caller owns instance uniqueness and serializes writes.
pub struct LibraryDatabase {
    pub(crate) connection: DatabaseConnection,
    path: PathBuf,
    identity: LibraryIdentity,
}
impl LibraryDatabase {
    /// Run on the library's runtime. Directory creation is a small blocking setup
    /// operation; this method must never be polled on a platform UI thread.
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
        let mut options = ConnectOptions::new("sqlite://library.sqlite");
        options
            .min_connections(1)
            .max_connections(4)
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts({
                let path = path.clone();
                move |o| {
                    o.filename(&path)
                        .create_if_missing(true)
                        .journal_mode(SqliteJournalMode::Wal)
                        .foreign_keys(true)
                        .busy_timeout(Duration::from_secs(5))
                        .synchronous(SqliteSynchronous::Full)
                }
            })
            .map_sqlx_sqlite_pool_opts(|o| {
                o.after_connect(|conn, _| {
                    Box::pin(async move {
                        let mode: String = sea_orm::sqlx::query_scalar("PRAGMA journal_mode")
                            .fetch_one(&mut *conn)
                            .await?;
                        if !mode.eq_ignore_ascii_case("wal") {
                            return Err(sea_orm::sqlx::Error::Protocol(
                                "WAL was not enabled".into(),
                            ));
                        }
                        Ok(())
                    })
                })
            });
        let connection = Database::connect(options).await?;
        let version = pragma(&connection, "PRAGMA user_version", "user_version").await?;
        let application = pragma(&connection, "PRAGMA application_id", "application_id").await?;
        if ![0, SCHEMA_VERSION].contains(&version)
            || (version > 0 && application != APPLICATION_ID)
            || (version == 0 && application != 0)
        {
            return Err(StorageError::UnsupportedSchema);
        }
        if version == 0 {
            let row = connection.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'".to_owned())).await?.ok_or(StorageError::Integrity("schema count missing"))?;
            let count: i64 = row.try_get("", "count")?;
            if count != 0 {
                return Err(StorageError::UnsupportedSchema);
            }
            schema::initialize(&connection).await?;
        }
        if pragma(&connection, "PRAGMA user_version", "user_version").await? != SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema);
        }
        let row = library_metadata::Entity::find_by_id(1_i64)
            .one(&connection)
            .await?
            .ok_or(StorageError::Integrity("library identity is missing"))?;
        let identity = LibraryIdentity {
            library_id: row.library_id.parse()?,
            device_id: row.device_id.parse()?,
        };
        Ok(Self {
            connection,
            path,
            identity,
        })
    }
    pub fn identity(&self) -> LibraryIdentity {
        self.identity
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub async fn close(self) -> Result<()> {
        self.connection.close().await?;
        Ok(())
    }
}
async fn pragma(db: &DatabaseConnection, sql: &str, field: &str) -> Result<i64> {
    let row = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await?
        .ok_or(StorageError::Integrity("pragma result missing"))?;
    Ok(row.try_get("", field)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_pool_connection_has_the_approved_pragmas()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let db = LibraryDatabase::open(dir.path().join("library.sqlite")).await?;
        let pool = db.connection.get_sqlite_connection_pool();
        let mut connections = Vec::new();
        for _ in 0..4 {
            connections.push(pool.acquire().await?);
        }
        for conn in &mut connections {
            let mode: String = sea_orm::sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&mut **conn)
                .await?;
            assert_eq!(mode, "wal");
            for (sql, expected) in [
                ("PRAGMA foreign_keys", 1_i64),
                ("PRAGMA busy_timeout", 5000),
                ("PRAGMA synchronous", 2),
            ] {
                let value: i64 = sea_orm::sqlx::query_scalar(sql)
                    .fetch_one(&mut **conn)
                    .await?;
                assert_eq!(value, expected, "{sql}");
            }
        }
        drop(connections);
        db.close().await?;
        Ok(())
    }
}
