use crate::{LibraryDatabase, Result, StorageError};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::{fs::File, path::Path};

impl LibraryDatabase {
    /// Consistent SQLite snapshot including committed WAL contents. Destination
    /// must be in a caller-controlled directory; SQLite refuses existing files.
    pub async fn snapshot(&self, destination: impl AsRef<Path>) -> Result<()> {
        snapshot(&self.connection, destination.as_ref()).await
    }
}
pub(super) async fn snapshot(db: &DatabaseConnection, destination: &Path) -> Result<()> {
    let name = destination
        .to_str()
        .ok_or(StorageError::InvalidInput("snapshot path is not UTF-8"))?;
    match destination.symlink_metadata() {
        Ok(_) => return Err(StorageError::Conflict("snapshot already exists")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "VACUUM INTO ?",
        [name.into()],
    ))
    .await?;
    File::open(destination)?.sync_all()?;
    if let Some(parent) = destination.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}
