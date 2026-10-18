//! Owned sync persistence; contains no credentials or network client.
use crate::{LibraryDatabase, Result, StorageError, transaction::WriteTransaction};
use memedock_domain::{
    archive::ArchiveData,
    local::LocalChange,
    sync::OperationEnvelope,
    version::{LocalOrder, Revision},
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub origin: String,
    pub library_id: memedock_domain::identity::LibraryId,
    pub epoch: memedock_domain::identity::SyncEpoch,
    pub device_id: memedock_domain::identity::DeviceId,
    pub cursor: Revision,
    pub cutoff: i64,
    pub enqueued: i64,
    pub initialized: bool,
    pub paused: bool,
    pub baseline: ArchiveData,
    pub canonical: ArchiveData,
    pub last_success_ms: Option<i64>,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncJob {
    pub ordinal: i64,
    pub source_order: Option<i64>,
    pub envelope: OperationEnvelope,
    pub attempted: bool,
    pub accepted: Option<Revision>,
    pub done: bool,
    #[serde(default)]
    pub projected: bool,
    pub error: Option<String>,
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|source| StorageError::InvalidData {
        table: "client_sync",
        source,
    })
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|source| StorageError::InvalidData {
        table: "client_sync",
        source,
    })
}
async fn binding(db: &impl ConnectionTrait, maximum: u64) -> Result<Option<Binding>> {
    let size = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT length(cast(payload AS BLOB)) AS bytes FROM client_sync WHERE singleton=1"
                .to_owned(),
        ))
        .await?;
    if let Some(size) = size {
        if size.try_get::<i64>("", "bytes")? as u64 > maximum {
            return Err(StorageError::InvalidInput("sync metadata exceeds budget"));
        }
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM client_sync WHERE singleton=1".to_owned(),
            ))
            .await?
            .ok_or(StorageError::Integrity("sync binding disappeared"))?;
        return decode(&row.try_get::<String>("", "payload")?).map(Some);
    }
    Ok(None)
}
impl LibraryDatabase {
    pub async fn sync_binding(&self, maximum: u64) -> Result<Option<Binding>> {
        binding(&self.connection, maximum).await
    }
    pub async fn sync_jobs(&self, maximum: u64) -> Result<Vec<SyncJob>> {
        let size = self.connection.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT coalesce(sum(length(cast(payload AS BLOB))),0) AS bytes FROM client_sync_jobs".to_owned())).await?.ok_or(StorageError::Integrity("sync job size"))?.try_get::<i64>("","bytes")?;
        if size > maximum as i64 {
            return Err(StorageError::InvalidInput("sync queue exceeds budget"));
        }
        self.connection
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT ordinal,source_order,payload FROM client_sync_jobs ORDER BY ordinal"
                    .to_owned(),
            ))
            .await?
            .into_iter()
            .map(|row| {
                let mut job: SyncJob = decode(&row.try_get::<String>("", "payload")?)?;
                job.ordinal = row.try_get("", "ordinal")?;
                job.source_order = row.try_get("", "source_order")?;
                Ok(job)
            })
            .collect()
    }
}
impl WriteTransaction {
    pub async fn sync_jobs(&self, maximum: u64) -> Result<Vec<SyncJob>> {
        let size = self.inner.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT coalesce(sum(length(cast(payload AS BLOB))),0) AS bytes FROM client_sync_jobs".to_owned())).await?.ok_or(StorageError::Integrity("sync job size"))?.try_get::<i64>("","bytes")?;
        if size as u64 > maximum {
            return Err(StorageError::InvalidInput("sync queue exceeds budget"));
        }
        self.inner
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT ordinal,source_order,payload FROM client_sync_jobs ORDER BY ordinal"
                    .to_owned(),
            ))
            .await?
            .into_iter()
            .map(|row| {
                let mut job: SyncJob = decode(&row.try_get::<String>("", "payload")?)?;
                job.ordinal = row.try_get("", "ordinal")?;
                job.source_order = row.try_get("", "source_order")?;
                Ok(job)
            })
            .collect()
    }
    pub async fn sync_binding(&self, maximum: u64) -> Result<Option<Binding>> {
        binding(&self.inner, maximum).await
    }
    pub async fn set_sync_binding(&mut self, binding: &Binding, maximum: u64) -> Result<()> {
        let payload = encode(binding)?;
        if payload.len() as u64 > maximum {
            return Err(StorageError::InvalidInput("sync metadata exceeds budget"));
        }
        self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,"INSERT INTO client_sync VALUES(1,?) ON CONFLICT(singleton) DO UPDATE SET payload=excluded.payload",[payload.into()])).await?;
        Ok(())
    }
    pub async fn sync_cutoff(&self) -> Result<i64> {
        Ok(self
            .inner
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT coalesce(max(local_order),0) AS n FROM local_changes".to_owned(),
            ))
            .await?
            .ok_or(StorageError::Integrity("local cutoff"))?
            .try_get("", "n")?)
    }
    pub async fn sync_source(&self, order: i64) -> Result<Option<LocalChange>> {
        use sea_orm::EntityTrait;
        crate::entities::local_change::Entity::find_by_id(order)
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn enqueue_sync(&mut self, job: &SyncJob, maximum: u64) -> Result<()> {
        let payload = encode(job)?;
        let size = self.inner.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT coalesce(sum(length(cast(payload AS BLOB))),0) AS bytes FROM client_sync_jobs".to_owned())).await?.ok_or(StorageError::Integrity("sync job size"))?.try_get::<i64>("","bytes")?;
        if (size as u64).saturating_add(payload.len() as u64) > maximum {
            return Err(StorageError::InvalidInput("sync queue exceeds budget"));
        }
        self.inner
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO client_sync_jobs(source_order,payload) VALUES(?,?)",
                [job.source_order.into(), payload.into()],
            ))
            .await?;
        Ok(())
    }
    pub async fn save_sync_job(&mut self, job: &SyncJob) -> Result<()> {
        let row = self
            .inner
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT payload FROM client_sync_jobs WHERE ordinal=?",
                [job.ordinal.into()],
            ))
            .await?
            .ok_or(StorageError::Conflict("sync job missing"))?;
        let old: SyncJob = decode(&row.try_get::<String>("", "payload")?)?;
        // Only a never-sent purge may bind its local deletion revision to the
        // server-assigned revision. All identity, generation and intent stay fixed.
        let prepared_purge = if !old.attempted {
            match (
                old.envelope.operation().kind(),
                job.envelope.operation().kind(),
            ) {
                (
                    memedock_domain::change::OperationKind::PurgeEntity {
                        entity: old_id,
                        generation: old_gen,
                        ..
                    },
                    memedock_domain::change::OperationKind::PurgeEntity {
                        entity, generation, ..
                    },
                ) if old_id == entity && old_gen == generation => {
                    old.envelope.library_id() == job.envelope.library_id()
                        && old.envelope.sync_epoch() == job.envelope.sync_epoch()
                        && old.envelope.device_id() == job.envelope.device_id()
                        && old.envelope.op_id() == job.envelope.op_id()
                        && old.envelope.client_time_ms() == job.envelope.client_time_ms()
                }
                _ => false,
            }
        } else {
            false
        };
        if (old.envelope != job.envelope && !prepared_purge) || old.source_order != job.source_order
        {
            return Err(StorageError::Conflict("frozen sync envelope"));
        }
        self.inner
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE client_sync_jobs SET payload=? WHERE ordinal=?",
                [encode(job)?.into(), job.ordinal.into()],
            ))
            .await?;
        Ok(())
    }
    pub async fn delete_finished_sync_jobs(&mut self) -> Result<()> {
        self.inner.execute_unprepared("DELETE FROM client_sync_jobs WHERE json_extract(payload,'$.done')=1 AND json_extract(payload,'$.error') IS NULL").await?;
        Ok(())
    }
    pub async fn reset_sync_jobs(&mut self) -> Result<()> {
        self.inner
            .execute_unprepared("DELETE FROM client_sync_jobs")
            .await?;
        Ok(())
    }
    pub async fn settle_local_history(&mut self, cutoff: i64) -> Result<()> {
        let mut after = None;
        loop {
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
            let rows = crate::entities::local_change::Entity::find()
                .filter(
                    crate::entities::local_change::Column::LocalOrder
                        .gt(after.map_or(0, LocalOrder::get)),
                )
                .filter(crate::entities::local_change::Column::LocalOrder.lte(cutoff))
                .order_by_asc(crate::entities::local_change::Column::LocalOrder)
                .limit(128)
                .all(&self.inner)
                .await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                let mut change = row.domain()?;
                after = Some(change.local_order());
                if change.contributes_overlay() {
                    if change.attempts() == 0 {
                        change.seal_local()?;
                    } else {
                        change.block("connection_replaced".into())?;
                    }
                    self.save_change(&change).await?;
                }
            }
        }
        Ok(())
    }
    /// Canonical state, outbox confirmation and cursor are committed together.
    /// This path deliberately does not append incoming events to the local outbox.
    pub async fn commit_sync(
        mut self,
        binding: &Binding,
        view: Option<&ArchiveData>,
        maximum: u64,
    ) -> Result<()> {
        if let Some(view) = view {
            view.validate()?;
            for asset in &view.assets {
                self.insert_asset(asset).await?;
                if self.local_asset(asset.hash()).await?.is_none() {
                    self.save_local_asset(&memedock_domain::local::LocalAsset::new(asset.hash()))
                        .await?;
                }
            }
            for entity in &view.stickers {
                self.save_sticker(entity).await?;
            }
            for entity in &view.collections {
                self.save_collection(entity).await?;
            }
            for entity in &view.tags {
                self.save_tag(entity).await?;
            }
            self.inner
                .execute_unprepared("DELETE FROM collection_items")
                .await?;
            self.inner
                .execute_unprepared("DELETE FROM sticker_tags")
                .await?;
            self.inner
                .execute_unprepared("DELETE FROM entity_tombstones")
                .await?;
            for entity in &view.collection_items {
                self.save_collection_item(entity).await?;
            }
            for entity in &view.sticker_tags {
                self.save_sticker_tag(entity).await?;
            }
            for entity in &view.tombstones {
                self.save_tombstone(entity).await?;
            }
        }
        self.set_sync_binding(binding, maximum).await?;
        if self.failed {
            return Err(StorageError::Conflict("failed sync transaction"));
        }
        self.inner.commit().await?;
        Ok(())
    }
}
