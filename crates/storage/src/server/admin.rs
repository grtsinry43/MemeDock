use super::{ServerDatabase, entities::admin};
use crate::{Result, StorageError};
use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Statement, TransactionTrait};

/// Owned credential material; intentionally has no Debug implementation.
pub struct AdminAccount {
    pub username: String,
    pub password_hash: String,
    pub auth_version: String,
}
impl ServerDatabase {
    pub async fn admin(&self) -> Result<Option<AdminAccount>> {
        Ok(admin::Entity::find_by_id(1)
            .one(&self.connection)
            .await?
            .map(|row| AdminAccount {
                username: row.username,
                password_hash: row.password_hash,
                auth_version: row.auth_version,
            }))
    }
    pub async fn initialize_admin(
        &self,
        username: &str,
        password_hash: &str,
        version: &str,
    ) -> Result<()> {
        let result = self.connection.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO server_admin(singleton,username,password_hash,auth_version) VALUES(1,?,?,?) ON CONFLICT(singleton) DO NOTHING",
            [username.into(), password_hash.into(), version.into()])).await?;
        if result.rows_affected() != 1 {
            return Err(StorageError::Conflict("administrator already initialized"));
        }
        Ok(())
    }
    /// Compare-and-set prevents an in-flight old password request undoing a reset.
    pub async fn replace_password(
        &self,
        expected_version: &str,
        hash: &str,
        version: &str,
    ) -> Result<()> {
        let tx = self.connection.begin().await?;
        let result = tx.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE server_admin SET password_hash=?,auth_version=? WHERE singleton=1 AND auth_version=?",
            [hash.into(), version.into(), expected_version.into()])).await?;
        if result.rows_affected() != 1 {
            return Err(StorageError::Conflict("administrator credentials changed"));
        }
        tx.execute_unprepared("DELETE FROM server_sessions").await?;
        tx.commit().await?;
        Ok(())
    }
}
