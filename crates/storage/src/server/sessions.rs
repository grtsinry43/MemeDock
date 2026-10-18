use super::{ServerDatabase, entities::session};
use crate::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};

pub struct StoredSession {
    pub payload: String,
    pub expires_at: i64,
}

impl ServerDatabase {
    /// Returns false on collision or when the account was reset during login.
    pub async fn create_session(
        &self,
        id: &str,
        payload: &str,
        expiry: i64,
        version: &str,
    ) -> Result<bool> {
        let result = self.connection.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO server_sessions(id_hash,payload,expires_at,auth_version) SELECT ?,?,?,? WHERE EXISTS(SELECT 1 FROM server_admin WHERE singleton=1 AND auth_version=?) ON CONFLICT(id_hash) DO NOTHING",
            [id.into(), payload.into(), expiry.into(), version.into(), version.into()])).await?;
        Ok(result.rows_affected() == 1)
    }
    /// Updating never resurrects a session deleted by logout/password reset.
    pub async fn save_session(&self, id: &str, payload: &str, expiry: i64) -> Result<()> {
        self.connection.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE server_sessions SET payload=?,expires_at=? WHERE id_hash=? AND auth_version=(SELECT auth_version FROM server_admin WHERE singleton=1)",
            [payload.into(), expiry.into(), id.into()])).await?;
        Ok(())
    }
    pub async fn load_session(&self, id: &str, now: i64) -> Result<Option<StoredSession>> {
        let row = session::Entity::find_by_id(id)
            .filter(session::Column::ExpiresAt.gt(now))
            .one(&self.connection)
            .await?;
        Ok(row.map(|row| StoredSession {
            payload: row.payload,
            expires_at: row.expires_at,
        }))
    }
    pub async fn delete_session(&self, id: &str) -> Result<()> {
        session::Entity::delete_by_id(id)
            .exec(&self.connection)
            .await?;
        Ok(())
    }
    pub async fn delete_expired_sessions(&self, now: i64) -> Result<()> {
        session::Entity::delete_many()
            .filter(session::Column::ExpiresAt.lte(now))
            .exec(&self.connection)
            .await?;
        Ok(())
    }
}
