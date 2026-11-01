use super::{decode, encode};
use crate::{LibraryDatabase, Result, transaction::WriteTransaction};
use memedock_domain::{asset::Asset, sync::diagnostics::SyncDiagnostic};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transfer {
    pub asset: Asset,
    pub attempts: u32,
    pub next_retry_ms: i64,
    pub blocked: bool,
    pub error: Option<SyncDiagnostic>,
}
impl Transfer {
    pub fn new(asset: Asset) -> Self {
        Self {
            asset,
            attempts: 0,
            next_retry_ms: 0,
            blocked: false,
            error: None,
        }
    }
}
impl LibraryDatabase {
    pub async fn sync_transfers(&self, maximum: u64) -> Result<Vec<Transfer>> {
        let row = self.connection.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT coalesce(sum(length(cast(payload AS BLOB))),0) AS bytes FROM client_sync_transfers".to_owned())).await?
            .ok_or(crate::StorageError::Integrity("transfer budget missing"))?;
        if row.try_get::<i64>("", "bytes")? as u64 > maximum {
            return Err(crate::StorageError::InvalidInput(
                "transfer queue exceeds budget",
            ));
        }
        self.connection
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM client_sync_transfers ORDER BY next_retry_ms,hash".to_owned(),
            ))
            .await?
            .into_iter()
            .map(|r| decode(&r.try_get::<String>("", "payload")?))
            .collect()
    }
}
impl WriteTransaction {
    pub async fn save_sync_transfer(&mut self, value: &Transfer) -> Result<()> {
        self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO client_sync_transfers(hash,next_retry_ms,payload) VALUES(?,?,?) ON CONFLICT(hash) DO UPDATE SET next_retry_ms=excluded.next_retry_ms,payload=excluded.payload",
            [value.asset.hash().to_string().into(), value.next_retry_ms.into(), encode(value)?.into()])).await?;
        Ok(())
    }
    pub async fn delete_sync_transfer(
        &mut self,
        hash: memedock_domain::identity::ContentHash,
    ) -> Result<()> {
        self.inner
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM client_sync_transfers WHERE hash=?",
                [hash.to_string().into()],
            ))
            .await?;
        Ok(())
    }
}
