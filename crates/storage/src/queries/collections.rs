use crate::{LibraryDatabase, Result, entities};
use memedock_domain::{
    collection::Collection,
    identity::{CollectionId, StickerId},
    relation::CollectionItem,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
impl LibraryDatabase {
    pub async fn sticker_collections(&self, id: StickerId) -> Result<Vec<Collection>> {
        use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement};
        let rows = self
            .connection
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT c.* FROM collections c JOIN collection_items r ON r.collection_id=c.id
             JOIN stickers s ON s.id=r.sticker_id WHERE s.id=? AND s.deleted_at IS NULL
             AND c.deleted_at IS NULL AND r.present=1 AND r.collection_generation=c.generation
             AND r.sticker_generation=s.generation ORDER BY c.sort_key COLLATE BINARY,c.id",
                [id.to_string().into()],
            ))
            .await?;
        rows.into_iter()
            .map(|row| entities::collection::Model::from_query_result(&row, "")?.domain())
            .collect()
    }
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
