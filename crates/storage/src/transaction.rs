use crate::{LibraryDatabase, Result, StorageError, entities};
use memedock_domain::{
    asset::Asset,
    change::Operation,
    collection::Collection,
    identity::{CollectionId, OperationId, StickerId, TagId},
    lifecycle::Tombstone,
    local::{LocalAsset, LocalChange, LocalUsage},
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    tag::Tag,
    version::{LocalOrder, TimestampMs},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseTransaction, EntityTrait, IntoActiveModel,
    SqliteTransactionMode, TransactionOptions, TransactionTrait,
};

/// Short write unit. Dropping it rolls back. Core owns domain transitions and
/// must append their operations before committing business changes.
pub struct WriteTransaction {
    pub(crate) inner: DatabaseTransaction,
    business_changed: bool,
    logged: bool,
    failed: bool,
}
impl LibraryDatabase {
    pub async fn begin_write(&self) -> Result<WriteTransaction> {
        let inner = self
            .connection
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await?;
        Ok(WriteTransaction {
            inner,
            business_changed: false,
            logged: false,
            failed: false,
        })
    }
}

macro_rules! save {
    ($method:ident,$module:ident,$domain:ty,$key:expr,$business:expr) => {
        pub async fn $method(&mut self, value: &$domain) -> Result<()> {
            let result = async {
                let row = entities::$module::Model::from_domain(value)?;
                let existing = entities::$module::Entity::find_by_id(($key)(value))
                    .one(&self.inner)
                    .await?;
                let active = row.into_active_model();
                if existing.is_some() {
                    active.reset_all().update(&self.inner).await?;
                } else {
                    active.insert(&self.inner).await?;
                }
                self.business_changed |= $business;
                Ok(())
            }
            .await;
            self.failed |= result.is_err();
            result
        }
    };
}
impl WriteTransaction {
    /// Identical bytes reuse their original metadata. A conflicting description
    /// of the same immutable asset is rejected; created_at is first-writer owned.
    pub async fn insert_asset(&mut self, asset: &Asset) -> Result<bool> {
        let result = self.insert_asset_inner(asset).await;
        self.failed |= result.is_err();
        result
    }
    async fn insert_asset_inner(&mut self, asset: &Asset) -> Result<bool> {
        if let Some(old) = entities::asset::Entity::find_by_id(asset.hash().to_string())
            .one(&self.inner)
            .await?
        {
            let old = old.domain()?;
            if old.byte_size() != asset.byte_size()
                || old.format() != asset.format()
                || old.width() != asset.width()
                || old.height() != asset.height()
                || old.animated() != asset.animated()
            {
                return Err(StorageError::Conflict("immutable asset metadata"));
            }
            return Ok(false);
        }
        entities::asset::Model::from_domain(asset)?
            .into_active_model()
            .insert(&self.inner)
            .await?;
        self.business_changed = true;
        Ok(true)
    }
    save!(
        save_sticker,
        sticker,
        Sticker,
        |v: &Sticker| v.id().to_string(),
        true
    );
    save!(
        save_collection,
        collection,
        Collection,
        |v: &Collection| v.id().to_string(),
        true
    );
    save!(save_tag, tag, Tag, |v: &Tag| v.id().to_string(), true);
    save!(
        save_collection_item,
        collection_item,
        CollectionItem,
        |v: &CollectionItem| (v.collection_id().to_string(), v.sticker_id().to_string()),
        true
    );
    save!(
        save_sticker_tag,
        sticker_tag,
        StickerTag,
        |v: &StickerTag| (v.sticker_id().to_string(), v.tag_id().to_string()),
        true
    );
    save!(
        save_tombstone,
        tombstone,
        Tombstone,
        |v: &Tombstone| match v.entity() {
            memedock_domain::lifecycle::EntityId::Sticker(id) =>
                ("sticker".to_owned(), id.to_string()),
            memedock_domain::lifecycle::EntityId::Collection(id) =>
                ("collection".to_owned(), id.to_string()),
            memedock_domain::lifecycle::EntityId::Tag(id) => ("tag".to_owned(), id.to_string()),
        },
        true
    );
    save!(
        save_local_asset,
        local_asset,
        LocalAsset,
        |v: &LocalAsset| v.hash().to_string(),
        false
    );
    save!(
        save_local_usage,
        local_usage,
        LocalUsage,
        |v: &LocalUsage| v.sticker_id().to_string(),
        false
    );

    pub async fn sticker(&self, id: StickerId) -> Result<Option<Sticker>> {
        entities::sticker::Entity::find_by_id(id.to_string())
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn local_usage(&self, id: StickerId) -> Result<Option<LocalUsage>> {
        entities::local_usage::Entity::find_by_id(id.to_string())
            .one(&self.inner)
            .await?
            .map(|row| row.domain())
            .transpose()
    }
    pub async fn collection(&self, id: CollectionId) -> Result<Option<Collection>> {
        entities::collection::Entity::find_by_id(id.to_string())
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn tag(&self, id: TagId) -> Result<Option<Tag>> {
        entities::tag::Entity::find_by_id(id.to_string())
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn collection_item(
        &self,
        collection: CollectionId,
        sticker: StickerId,
    ) -> Result<Option<CollectionItem>> {
        entities::collection_item::Entity::find_by_id((collection.to_string(), sticker.to_string()))
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn sticker_tag(&self, sticker: StickerId, tag: TagId) -> Result<Option<StickerTag>> {
        entities::sticker_tag::Entity::find_by_id((sticker.to_string(), tag.to_string()))
            .one(&self.inner)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn append_change(
        &mut self,
        op_id: OperationId,
        operation: Operation,
        at: TimestampMs,
    ) -> Result<LocalChange> {
        let result = self.append_change_inner(op_id, operation, at).await;
        self.failed |= result.is_err();
        result
    }
    async fn append_change_inner(
        &mut self,
        op_id: OperationId,
        operation: Operation,
        at: TimestampMs,
    ) -> Result<LocalChange> {
        // SQLite allocates the order inside this transaction, not via MAX()+1.
        let provisional = LocalChange::new(LocalOrder::new(1)?, op_id, operation.clone(), at);
        let mut active =
            entities::local_change::Model::from_domain(&provisional)?.into_active_model();
        active.local_order = ActiveValue::NotSet;
        let row = active.insert(&self.inner).await?;
        self.logged = true;
        Ok(LocalChange::new(
            LocalOrder::new(row.local_order)?,
            op_id,
            operation,
            at,
        ))
    }
    /// Persist a domain-validated outbox transition. Attempted payloads cannot be
    /// rewritten even if a caller constructs a different valid LocalChange.
    pub async fn save_change(&mut self, change: &LocalChange) -> Result<()> {
        let result = self.save_change_inner(change).await;
        self.failed |= result.is_err();
        result
    }
    async fn save_change_inner(&mut self, change: &LocalChange) -> Result<()> {
        let old = entities::local_change::Entity::find_by_id(change.local_order().get())
            .one(&self.inner)
            .await?
            .ok_or(StorageError::Conflict("unknown local change"))?
            .domain()?;
        if old.op_id() != change.op_id()
            || old.created_at() != change.created_at()
            || change.attempts() < old.attempts()
            || (old.attempts() > 0 && old.operation() != change.operation())
        {
            return Err(StorageError::Conflict(
                "frozen local change identity or payload",
            ));
        }
        let active = entities::local_change::Model::from_domain(change)?.into_active_model();
        active.reset_all().update(&self.inner).await?;
        Ok(())
    }
    pub async fn commit(self) -> Result<()> {
        if self.failed {
            self.inner.rollback().await?;
            return Err(StorageError::Conflict(
                "a failed write transaction cannot commit",
            ));
        }
        if self.business_changed && !self.logged {
            self.inner.rollback().await?;
            return Err(StorageError::Conflict(
                "business changes require an operation log",
            ));
        }
        self.inner.commit().await?;
        Ok(())
    }
    pub async fn rollback(self) -> Result<()> {
        self.inner.rollback().await?;
        Ok(())
    }
}
