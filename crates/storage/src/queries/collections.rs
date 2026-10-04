use crate::{LibraryDatabase, Result, entities};
use memedock_domain::{
    collection::Collection,
    identity::{CollectionId, StickerId},
    relation::CollectionItem,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
impl LibraryDatabase {
    pub async fn collection(&self, id: CollectionId) -> Result<Option<Collection>> {
        entities::collection::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn collections(&self, deleted: bool) -> Result<Vec<Collection>> {
        let col = entities::collection::Column::DeletedAt;
        let rows = entities::collection::Entity::find()
            .filter(if deleted {
                col.is_not_null()
            } else {
                col.is_null()
            })
            .order_by_asc(entities::collection::Column::SortKey)
            .order_by_asc(entities::collection::Column::Id)
            .all(&self.connection)
            .await?;
        rows.into_iter().map(|r| r.domain()).collect()
    }
    /// Historical relation, for explicit recovery suggestions; not display data.
    pub async fn collection_item(
        &self,
        collection: CollectionId,
        sticker: StickerId,
    ) -> Result<Option<CollectionItem>> {
        entities::collection_item::Entity::find_by_id((collection.to_string(), sticker.to_string()))
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
}
