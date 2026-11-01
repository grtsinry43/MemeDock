use super::{ServerDatabase, entities::device};
use crate::{Result, StorageError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, Set, SqliteTransactionMode, Statement, TransactionOptions, TransactionTrait,
};

pub struct StoredDevice {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    pub last_seen_at: Option<i64>,
}
impl From<device::Model> for StoredDevice {
    fn from(row: device::Model) -> Self {
        Self {
            id: row.id,
            name: row.name,
            token_hash: row.token_hash,
            created_at: row.created_at,
            revoked_at: row.revoked_at,
            last_seen_at: row.last_seen_at,
        }
    }
}
impl ServerDatabase {
    pub async fn create_device(
        &self,
        id: &str,
        name: &str,
        hash: &str,
        now: i64,
    ) -> Result<StoredDevice> {
        let row = device::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            token_hash: Set(hash.into()),
            created_at: Set(now),
            revoked_at: Set(None),
            last_seen_at: Set(None),
        }
        .insert(&self.connection)
        .await?;
        Ok(row.into())
    }
    pub async fn devices(&self) -> Result<Vec<StoredDevice>> {
        Ok(device::Entity::find()
            .order_by_desc(device::Column::CreatedAt)
            .order_by_asc(device::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub async fn device(&self, id: &str) -> Result<Option<StoredDevice>> {
        Ok(device::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(Into::into))
    }
    pub async fn revoke_device(&self, id: &str, now: i64) -> Result<bool> {
        let result = self
            .connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE server_devices SET revoked_at=COALESCE(revoked_at,?) WHERE id=?",
                [now.into(), id.into()],
            ))
            .await?;
        Ok(result.rows_affected() == 1)
    }
    pub async fn mark_device_seen(&self, id: &str, now: i64) -> Result<bool> {
        let result = device::Entity::update_many()
            .col_expr(
                device::Column::LastSeenAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(device::Column::Id.eq(id))
            .filter(device::Column::RevokedAt.is_null())
            .exec(&self.connection)
            .await?;
        Ok(result.rows_affected == 1)
    }
    /// Remove credentials only after revocation. Canonical entities, events
    /// and operation receipts remain server-owned and are never cascaded away.
    pub async fn delete_revoked_device(&self, id: &str) -> Result<bool> {
        let tx = self
            .connection
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await?;
        let row = tx
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT revoked_at FROM server_devices WHERE id=?",
                [id.into()],
            ))
            .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        if row.try_get::<Option<i64>>("", "revoked_at")?.is_none() {
            return Err(StorageError::Conflict(
                "revoke the device before deleting it",
            ));
        }
        tx.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM server_device_progress WHERE device_id=?",
            [id.into()],
        ))
        .await?;
        let deleted = tx
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM server_devices WHERE id=? AND revoked_at IS NOT NULL",
                [id.into()],
            ))
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(StorageError::Conflict("device changed during deletion"));
        }
        tx.commit().await?;
        Ok(true)
    }
}
