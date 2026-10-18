use super::{ServerDatabase, browse::page_bounds, sync::identity};
use crate::{Result, StorageError};
use memedock_domain::identity::DeviceId;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value};

pub struct DeviceProgress {
    pub id: String,
    pub name: String,
    pub revoked: bool,
    pub last_seen_at: Option<i64>,
    pub applied_cursor: Option<i64>,
    pub observed_at: Option<i64>,
}
pub struct SyncOverview {
    pub head: i64,
    pub stickers: u64,
    pub deleted: u64,
    pub collections: u64,
    pub tags: u64,
    pub original_bytes: u64,
    pub devices: Vec<DeviceProgress>,
}
pub struct ActivityFilter {
    pub device: Option<DeviceId>,
    pub result: Option<String>,
    pub page: u32,
    pub limit: u32,
}
pub struct ActivityEntry {
    pub op_id: String,
    pub device_id: String,
    pub device_name: String,
    pub kind: String,
    pub target: Option<String>,
    pub result: String,
    pub error_code: Option<String>,
    pub received_at: Option<i64>,
    pub event_seq: Option<i64>,
}
pub struct ActivityPage {
    pub items: Vec<ActivityEntry>,
    pub total: u64,
}

impl ServerDatabase {
    pub async fn sync_overview(&self) -> Result<SyncOverview> {
        let tx = self.connection.begin().await?;
        let identity = identity(&tx).await?;
        let row = tx.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT coalesce(sum(kind='sticker' AND json_extract(payload,'$.deleted_at') IS NULL),0) AS stickers,coalesce(sum(kind='sticker' AND json_extract(payload,'$.deleted_at') IS NOT NULL AND json_extract(payload,'$.purged_at') IS NULL),0) AS deleted,coalesce(sum(kind='collection' AND json_extract(payload,'$.deleted_at') IS NULL),0) AS collections,coalesce(sum(kind='tag' AND json_extract(payload,'$.deleted_at') IS NULL),0) AS tags,(SELECT coalesce(sum(json_extract(a.payload,'$.byte_size')),0) FROM server_assets a WHERE NOT EXISTS(SELECT 1 FROM server_sync_entities s WHERE s.kind='sticker' AND s.id=a.hash AND json_extract(s.payload,'$.purged_at') IS NOT NULL)) AS bytes FROM server_sync_entities".to_owned())).await?.ok_or(StorageError::Integrity("overview missing"))?;
        let rows = tx.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT d.id,d.name,d.revoked_at,d.last_seen_at,p.applied_cursor,p.observed_at FROM server_devices d LEFT JOIN server_device_progress p ON p.device_id=d.id AND p.epoch=? ORDER BY d.created_at DESC,d.id LIMIT 1001", [identity.epoch.to_string().into()])).await?;
        if rows.len() > 1000 {
            return Err(StorageError::InvalidInput("too many devices for overview"));
        }
        let devices = rows
            .into_iter()
            .map(|r| {
                Ok(DeviceProgress {
                    id: r.try_get("", "id")?,
                    name: r.try_get("", "name")?,
                    revoked: r.try_get::<Option<i64>>("", "revoked_at")?.is_some(),
                    last_seen_at: r.try_get("", "last_seen_at")?,
                    applied_cursor: r.try_get("", "applied_cursor")?,
                    observed_at: r.try_get("", "observed_at")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let result = SyncOverview {
            head: identity.head.get(),
            stickers: row.try_get::<i64>("", "stickers")? as u64,
            deleted: row.try_get::<i64>("", "deleted")? as u64,
            collections: row.try_get::<i64>("", "collections")? as u64,
            tags: row.try_get::<i64>("", "tags")? as u64,
            original_bytes: row.try_get::<i64>("", "bytes")? as u64,
            devices,
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Only persisted receipts are exposed; never return request bodies or credentials.
    pub async fn sync_activity(&self, filter: &ActivityFilter) -> Result<ActivityPage> {
        page_bounds(filter.page, filter.limit, 100)?;
        if filter
            .result
            .as_deref()
            .is_some_and(|v| !["accepted", "no_op", "rejected"].contains(&v))
        {
            return Err(StorageError::InvalidInput("receipt filter"));
        }
        let mut condition = "1=1".to_owned();
        let mut values: Vec<Value> = Vec::new();
        if let Some(device) = filter.device {
            condition.push_str(" AND r.device_id=?");
            values.push(device.to_string().into());
        }
        if let Some(result) = &filter.result {
            condition.push_str(" AND json_extract(r.receipt,'$.status')=?");
            values.push(result.clone().into());
        }
        let tx = self.connection.begin().await?;
        let total = tx
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!("SELECT count(*) AS n FROM server_sync_receipts r WHERE {condition}"),
                values.clone(),
            ))
            .await?
            .ok_or(StorageError::Integrity("receipt count"))?
            .try_get::<i64>("", "n")? as u64;
        values.push(i64::from(filter.limit).into());
        values.push((i64::from(filter.page - 1) * i64::from(filter.limit)).into());
        let rows = tx.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite, format!("SELECT r.device_id,r.op_id,d.name,r.received_at,json_extract(r.request,'$.operation.type') AS kind,coalesce(json_extract(r.request,'$.operation.sticker_id'),json_extract(r.request,'$.operation.collection_id'),json_extract(r.request,'$.operation.tag_id'),json_extract(r.request,'$.operation.asset.hash')) AS target,json_extract(r.receipt,'$.status') AS result,json_extract(r.receipt,'$.code') AS error_code,json_extract(r.receipt,'$.event_seq') AS event_seq FROM server_sync_receipts r JOIN server_devices d ON d.id=r.device_id WHERE {condition} ORDER BY r.received_at DESC,r.device_id,r.op_id LIMIT ? OFFSET ?"), values)).await?;
        let items = rows
            .into_iter()
            .map(|r| {
                Ok(ActivityEntry {
                    op_id: r.try_get("", "op_id")?,
                    device_id: r.try_get("", "device_id")?,
                    device_name: r.try_get("", "name")?,
                    kind: r.try_get("", "kind")?,
                    target: r.try_get("", "target")?,
                    result: r.try_get("", "result")?,
                    error_code: r.try_get("", "error_code")?,
                    received_at: r.try_get("", "received_at")?,
                    event_seq: r.try_get("", "event_seq")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(ActivityPage { items, total })
    }
}
