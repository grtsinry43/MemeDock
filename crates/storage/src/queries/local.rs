use crate::{LibraryDatabase, Result, StorageError, entities};
use memedock_domain::{
    identity::{ContentHash, StickerId},
    lifecycle::{EntityId, Tombstone},
    local::{LocalAsset, LocalChange, LocalUsage},
    version::LocalOrder,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement,
};

#[derive(Debug, PartialEq, Eq)]
pub struct SpaceStatistics {
    pub known_assets: i64,
    pub known_original_bytes: i64,
    pub ready_original_bytes: i64,
}
impl LibraryDatabase {
    /// Enumerate all known originals, including soft-deleted entities, for an
    /// explicit recovery scan. No physical garbage collection is implied.
    pub async fn asset_hashes_after(
        &self,
        after: Option<ContentHash>,
        limit: u32,
    ) -> Result<Vec<ContentHash>> {
        if !(1..=200).contains(&limit) {
            return Err(StorageError::InvalidInput(
                "asset page size must be 1..=200",
            ));
        }
        let mut query = entities::asset::Entity::find()
            .select_only()
            .column(entities::asset::Column::Hash);
        if let Some(after) = after {
            query = query.filter(entities::asset::Column::Hash.gt(after.to_string()));
        }
        query
            .order_by_asc(entities::asset::Column::Hash)
            .limit(u64::from(limit))
            .into_tuple::<String>()
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|hash| hash.parse().map_err(StorageError::from))
            .collect()
    }
    pub async fn local_asset(&self, hash: ContentHash) -> Result<Option<LocalAsset>> {
        entities::local_asset::Entity::find_by_id(hash.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn local_usage(&self, id: StickerId) -> Result<Option<LocalUsage>> {
        entities::local_usage::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn changes_after(
        &self,
        after: Option<LocalOrder>,
        limit: u32,
    ) -> Result<Vec<LocalChange>> {
        if !(1..=200).contains(&limit) {
            return Err(StorageError::InvalidInput(
                "change page size must be 1..=200",
            ));
        }
        entities::local_change::Entity::find()
            .filter(entities::local_change::Column::LocalOrder.gt(after.map_or(0, LocalOrder::get)))
            .order_by_asc(entities::local_change::Column::LocalOrder)
            .limit(u64::from(limit))
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|r| r.domain())
            .collect()
    }
    pub async fn tombstone(&self, id: EntityId) -> Result<Option<Tombstone>> {
        let key = match id {
            EntityId::Sticker(id) => ("sticker".to_owned(), id.to_string()),
            EntityId::Collection(id) => ("collection".to_owned(), id.to_string()),
            EntityId::Tag(id) => ("tag".to_owned(), id.to_string()),
        };
        entities::tombstone::Entity::find_by_id(key)
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    /// Logical known/ready byte totals, not a filesystem allocation measurement.
    pub async fn space_statistics(&self) -> Result<SpaceStatistics> {
        let row=self.connection.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT count(*) AS known_assets,coalesce(sum(a.byte_size),0) AS known_original_bytes,coalesce(sum(CASE WHEN l.blob_status='ready' THEN a.byte_size ELSE 0 END),0) AS ready_original_bytes FROM assets a LEFT JOIN local_assets l ON l.hash=a.hash".to_owned()))
            .await?.ok_or(StorageError::Integrity("space query missing"))?;
        Ok(SpaceStatistics {
            known_assets: row.try_get("", "known_assets")?,
            known_original_bytes: row.try_get("", "known_original_bytes")?,
            ready_original_bytes: row.try_get("", "ready_original_bytes")?,
        })
    }
}
