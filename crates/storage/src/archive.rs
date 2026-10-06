use crate::{LibraryDatabase, Result, entities};
use memedock_domain::archive::ArchiveData;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement,
};

impl LibraryDatabase {
    /// Bound rows and text before allocating ORM models for an archive operation.
    pub async fn archive_data_bounded(&self, maximum_bytes: u64) -> Result<ArchiveData> {
        let sql="SELECT (
            (SELECT count(*) FROM assets)+(SELECT count(*) FROM stickers)+
            (SELECT count(*) FROM collections)+(SELECT count(*) FROM tags)+
            (SELECT count(*) FROM collection_items)+(SELECT count(*) FROM sticker_tags)+
            (SELECT count(*) FROM entity_tombstones)) * 512 + 6 * (
            (SELECT coalesce(sum(length(cast(title AS BLOB))+length(cast(note AS BLOB))+length(cast(original_name AS BLOB))),0) FROM stickers)+
            (SELECT coalesce(sum(length(cast(name AS BLOB))),0) FROM collections)+
            (SELECT coalesce(sum(length(cast(name AS BLOB))),0) FROM tags)) AS estimate";
        let row = self
            .connection
            .query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
            .await?
            .ok_or(crate::StorageError::Integrity("archive budget missing"))?;
        let bytes: i64 = row.try_get("", "estimate")?;
        if bytes < 0 || bytes as u64 > maximum_bytes {
            return Err(crate::StorageError::InvalidInput(
                "archive metadata exceeds budget",
            ));
        }
        self.archive_data().await
    }
    /// Call against a consistent snapshot or while holding core's write permit.
    pub async fn archive_data(&self) -> Result<ArchiveData> {
        macro_rules! rows {
            ($entity:ident, $column:ident) => {
                entities::$entity::Entity::find()
                    .order_by_asc(entities::$entity::Column::$column)
                    .all(&self.connection)
                    .await?
                    .into_iter()
                    .map(|row| row.domain())
                    .collect::<Result<Vec<_>>>()?
            };
        }
        let mut data = ArchiveData {
            assets: rows!(asset, Hash),
            stickers: rows!(sticker, Id),
            collections: rows!(collection, Id),
            tags: rows!(tag, Id),
            collection_items: rows!(collection_item, CollectionId),
            sticker_tags: rows!(sticker_tag, StickerId),
            tombstones: rows!(tombstone, EntityId),
        };
        // Local delivery-only assets are not part of the portable business library.
        let ids: std::collections::BTreeSet<_> = data
            .stickers
            .iter()
            .map(|v| v.id().content_hash())
            .collect();
        data.assets.retain(|v| ids.contains(&v.hash()));
        data.validate()?;
        Ok(data)
    }

    /// Preserve current handoff records/pins in a replacement DB. Their source
    /// metadata and bytes must survive even when absent from the portable archive.
    pub async fn copy_delivery_state_from(&self, source: &LibraryDatabase) -> Result<()> {
        use sea_orm::TransactionTrait;
        let tx = self.connection.begin().await?;
        use sea_orm::{ActiveModelTrait, IntoActiveModel};
        let mut after = None;
        loop {
            let mut query = entities::export_artifact::Entity::find()
                .order_by_asc(entities::export_artifact::Column::Id)
                .limit(256);
            if let Some(id) = after {
                query = query.filter(entities::export_artifact::Column::Id.gt(id));
            }
            let batch = query.all(&source.connection).await?;
            if batch.is_empty() {
                break;
            }
            after = batch.last().map(|v| v.id.clone());
            for artifact in batch {
                if entities::asset::Entity::find_by_id(artifact.source_hash.clone())
                    .one(&tx)
                    .await?
                    .is_none()
                {
                    let asset = entities::asset::Entity::find_by_id(artifact.source_hash.clone())
                        .one(&source.connection)
                        .await?
                        .ok_or(crate::StorageError::Integrity(
                            "delivery original metadata missing",
                        ))?;
                    asset.into_active_model().insert(&tx).await?;
                }
                artifact.into_active_model().insert(&tx).await?;
            }
        }
        let mut after = None;
        loop {
            let mut query = entities::clipboard_reference::Entity::find()
                .order_by_asc(entities::clipboard_reference::Column::Id)
                .limit(256);
            if let Some(id) = after {
                query = query.filter(entities::clipboard_reference::Column::Id.gt(id));
            }
            let batch = query.all(&source.connection).await?;
            if batch.is_empty() {
                break;
            }
            after = batch.last().map(|v| v.id.clone());
            for pin in batch {
                pin.into_active_model().insert(&tx).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn archive_target_revision(&self) -> Result<i64> {
        let row = self
            .connection
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT coalesce(max(local_order),0) AS revision FROM local_changes".to_owned(),
            ))
            .await?
            .ok_or(crate::StorageError::Integrity("local revision missing"))?;
        Ok(row.try_get("", "revision")?)
    }
}

impl crate::transaction::WriteTransaction {
    pub async fn import_archive(&mut self, data: &ArchiveData) -> Result<()> {
        data.validate()?;
        for v in &data.assets {
            self.insert_asset(v).await?;
        }
        for v in &data.stickers {
            self.save_sticker(v).await?;
        }
        for v in &data.collections {
            self.save_collection(v).await?;
        }
        for v in &data.tags {
            self.save_tag(v).await?;
        }
        for v in &data.collection_items {
            self.save_collection_item(v).await?;
        }
        for v in &data.sticker_tags {
            self.save_sticker_tag(v).await?;
        }
        for v in &data.tombstones {
            self.save_tombstone(v).await?;
        }
        Ok(())
    }
}
