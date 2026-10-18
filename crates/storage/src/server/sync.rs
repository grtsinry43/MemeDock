use super::ServerDatabase;
use crate::{Result, StorageError};
use memedock_domain::{
    asset::Asset,
    identity::{ContentHash, DeviceId, LibraryId, SyncEpoch},
    sync::{OperationEnvelope, OperationReceipt, PullPage, SyncEvent},
    version::{Revision, TimestampMs},
};
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbBackend, SqliteTransactionMode, Statement,
    TransactionOptions, TransactionTrait,
};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug)]
pub struct SyncIdentity {
    pub library_id: LibraryId,
    pub epoch: SyncEpoch,
    pub head: Revision,
}
pub struct SyncTransaction {
    inner: DatabaseTransaction,
    pub identity: SyncIdentity,
}
pub(super) fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|source| StorageError::InvalidData {
        table: "server_sync",
        source,
    })
}
pub(super) fn decode<T: DeserializeOwned>(json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|source| StorageError::InvalidData {
        table: "server_sync",
        source,
    })
}
pub(super) async fn identity(db: &impl ConnectionTrait) -> Result<SyncIdentity> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT library_id,epoch,head FROM server_sync_library WHERE singleton=1".to_owned(),
        ))
        .await?
        .ok_or(StorageError::Integrity("sync identity missing"))?;
    Ok(SyncIdentity {
        library_id: row.try_get::<String>("", "library_id")?.parse()?,
        epoch: row.try_get::<String>("", "epoch")?.parse()?,
        head: Revision::new(row.try_get("", "head")?)?,
    })
}
pub(super) async fn read_asset(
    db: &impl ConnectionTrait,
    hash: ContentHash,
) -> Result<Option<Asset>> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT payload FROM server_assets WHERE hash=?",
        [hash.to_string().into()],
    ))
    .await?
    .map(|r| decode(&r.try_get::<String>("", "payload")?))
    .transpose()
}
impl ServerDatabase {
    pub async fn sync_identity(&self) -> Result<SyncIdentity> {
        identity(&self.connection).await
    }
    pub async fn begin_sync(&self, device: DeviceId) -> Result<SyncTransaction> {
        let inner = self
            .connection
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await?;
        let active = inner
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT id FROM server_devices WHERE id=? AND revoked_at IS NULL",
                [device.to_string().into()],
            ))
            .await?;
        if active.is_none() {
            return Err(StorageError::Conflict("device revoked"));
        }
        let identity = identity(&inner).await?;
        Ok(SyncTransaction { inner, identity })
    }
    pub async fn pull_events(
        &self,
        device: DeviceId,
        library: LibraryId,
        epoch: SyncEpoch,
        after: Revision,
        limit: u32,
    ) -> Result<PullPage> {
        self.pull_events_observed(device, (library, epoch), after, limit, None)
            .await
    }
    /// `after` acknowledges already-applied events; the returned page is not an acknowledgement.
    pub async fn pull_events_observed(
        &self,
        device: DeviceId,
        identity: (LibraryId, SyncEpoch),
        after: Revision,
        limit: u32,
        observed: Option<TimestampMs>,
    ) -> Result<PullPage> {
        let (library, epoch) = identity;
        if !(1..=256).contains(&limit) {
            return Err(StorageError::InvalidInput("page limit"));
        }
        let tx = self.begin_sync(device).await?;
        if library != tx.identity.library_id
            || epoch != tx.identity.epoch
            || after > tx.identity.head
        {
            return Err(StorageError::Conflict("sync identity or cursor"));
        }
        let rows = tx
            .inner
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT payload FROM server_sync_events WHERE seq>? ORDER BY seq LIMIT ?",
                [after.get().into(), (i64::from(limit) + 1).into()],
            ))
            .await?;
        let more = rows.len() > limit as usize;
        let events = rows
            .into_iter()
            .take(limit as usize)
            .map(|r| decode::<SyncEvent>(&r.try_get::<String>("", "payload")?))
            .collect::<Result<Vec<_>>>()?;
        let page = PullPage::new(library, epoch, after, events, more)?;
        if let Some(at) = observed {
            tx.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
                "INSERT INTO server_device_progress VALUES(?,?,?,?) ON CONFLICT(device_id) DO UPDATE SET epoch=excluded.epoch,applied_cursor=excluded.applied_cursor,observed_at=excluded.observed_at",
                [device.to_string().into(), epoch.to_string().into(), after.get().into(), at.get().into()])).await?;
        }
        tx.inner.commit().await?;
        Ok(page)
    }
}
impl SyncTransaction {
    pub async fn queue_purge(
        &self,
        id: memedock_domain::identity::StickerId,
        generation: memedock_domain::version::Generation,
    ) -> Result<()> {
        self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO server_purge_cleanup(hash,generation) VALUES(?,?) ON CONFLICT(hash) DO UPDATE SET generation=excluded.generation",
            [id.to_string().into(), generation.get().into()])).await?;
        Ok(())
    }
    pub async fn prior_receipt(&self, op: &OperationEnvelope) -> Result<Option<OperationReceipt>> {
        let row = self
            .inner
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT request,receipt FROM server_sync_receipts WHERE device_id=? AND op_id=?",
                [
                    op.device_id().to_string().into(),
                    op.op_id().to_string().into(),
                ],
            ))
            .await?;
        row.map(|r| {
            let request: String = r.try_get("", "request")?;
            if request != encode(op)? {
                return Err(StorageError::Conflict("op id reused"));
            }
            decode(&r.try_get::<String>("", "receipt")?)
        })
        .transpose()
    }
    pub async fn record<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<Option<T>> {
        self.inner
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT payload FROM server_sync_entities WHERE kind=? AND id=?",
                [kind.into(), id.into()],
            ))
            .await?
            .map(|r| decode(&r.try_get::<String>("", "payload")?))
            .transpose()
    }
    pub async fn save(
        &self,
        kind: &str,
        id: &str,
        value: &impl Serialize,
        sort: Option<&str>,
        parent: Option<&str>,
    ) -> Result<()> {
        self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO server_sync_entities(kind,id,payload,sort_key,parent_id) VALUES(?,?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET payload=excluded.payload,sort_key=excluded.sort_key,parent_id=excluded.parent_id",
            [kind.into(), id.into(), encode(value)?.into(), sort.map(str::to_owned).into(), parent.map(str::to_owned).into()])).await?;
        Ok(())
    }
    pub async fn asset(&self, hash: ContentHash) -> Result<Option<Asset>> {
        read_asset(&self.inner, hash).await
    }
    /// Only effective rows participate in ordering. No whole-library materialization.
    pub async fn neighbors(
        &self,
        kind: &str,
        parent: Option<&str>,
        before: Option<&str>,
        exclude: &str,
    ) -> Result<(Option<String>, Option<String>, bool)> {
        let filter = "kind=? AND parent_id IS ? AND id!=? AND (CASE WHEN kind='collection' THEN json_extract(payload,'$.deleted_at') IS NULL ELSE json_extract(payload,'$.present')=1 AND EXISTS(SELECT 1 FROM server_sync_entities s WHERE s.kind='sticker' AND s.id=json_extract(server_sync_entities.payload,'$.sticker_id') AND json_extract(s.payload,'$.deleted_at') IS NULL AND json_extract(s.payload,'$.generation')=json_extract(server_sync_entities.payload,'$.sticker_generation')) AND EXISTS(SELECT 1 FROM server_sync_entities c WHERE c.kind='collection' AND c.id=server_sync_entities.parent_id AND json_extract(c.payload,'$.deleted_at') IS NULL AND json_extract(c.payload,'$.generation')=json_extract(server_sync_entities.payload,'$.collection_generation')) END)";
        let mut right = None;
        if let Some(id) = before {
            let sql = format!("SELECT sort_key FROM server_sync_entities WHERE {filter} AND id=?");
            right = self
                .inner
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    [
                        kind.into(),
                        parent.map(str::to_owned).into(),
                        exclude.into(),
                        id.into(),
                    ],
                ))
                .await?
                .map(|r| r.try_get::<String>("", "sort_key"))
                .transpose()?;
        }
        let sql = format!(
            "SELECT sort_key FROM server_sync_entities WHERE {filter} {} ORDER BY sort_key COLLATE BINARY DESC,id DESC LIMIT 1",
            if right.is_some() {
                "AND sort_key<?"
            } else {
                ""
            }
        );
        let mut args = vec![
            kind.into(),
            parent.map(str::to_owned).into(),
            exclude.into(),
        ];
        if let Some(r) = &right {
            args.push(r.clone().into());
        }
        let left = self
            .inner
            .query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite, sql, args))
            .await?
            .map(|r| r.try_get("", "sort_key"))
            .transpose()?;
        let missing = before.is_some() && right.is_none();
        Ok((left, right, missing))
    }
    pub async fn finish(
        self,
        op: &OperationEnvelope,
        receipt: &OperationReceipt,
        event: Option<&SyncEvent>,
        received_at: TimestampMs,
    ) -> Result<()> {
        if let Some(event) = event {
            if event.seq().get() != self.identity.head.checked_next()?.get() {
                return Err(StorageError::Integrity("event sequence"));
            }
            self.inner
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO server_sync_events VALUES(?,?)",
                    [event.seq().get().into(), encode(event)?.into()],
                ))
                .await?;
            self.inner
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "UPDATE server_sync_library SET head=? WHERE singleton=1",
                    [event.seq().get().into()],
                ))
                .await?;
        }
        self.inner
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO server_sync_receipts(device_id,op_id,request,receipt,received_at) VALUES(?,?,?,?,?)",
                [
                    op.device_id().to_string().into(),
                    op.op_id().to_string().into(),
                    encode(op)?.into(),
                    encode(receipt)?.into(),
                    received_at.get().into(),
                ],
            ))
            .await?;
        self.inner.commit().await?;
        Ok(())
    }
}
