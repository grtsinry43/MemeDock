use crate::{LibraryDatabase, Result, entities};
use memedock_domain::{
    collection::Collection,
    identity::{CollectionId, StickerId},
    relation::CollectionItem,
};
use sea_orm::{ColumnTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, Statement};
pub struct CollectionSummary {
    pub collection: Collection,
    pub count: u64,
    pub cover: Option<StickerId>,
}
impl LibraryDatabase {
    pub async fn collection_summaries(&self) -> Result<Vec<CollectionSummary>> {
        use sea_orm::{ConnectionTrait, FromQueryResult};
        let sql = "WITH effective AS (
            SELECT ci.collection_id, ci.sticker_id, ci.sort_key FROM collection_items ci
            JOIN collections c ON c.id=ci.collection_id JOIN stickers s ON s.id=ci.sticker_id
            WHERE ci.present=1 AND c.deleted_at IS NULL AND s.deleted_at IS NULL
            AND ci.collection_generation=c.generation AND ci.sticker_generation=s.generation
        ), ranked AS (
            SELECT collection_id, sticker_id, count(*) OVER(PARTITION BY collection_id) AS item_count,
            row_number() OVER(PARTITION BY collection_id ORDER BY sort_key COLLATE BINARY,sticker_id) AS position FROM effective
        ) SELECT c.*,coalesce(r.item_count,0) AS item_count,r.sticker_id AS cover_id
          FROM collections c LEFT JOIN ranked r ON r.collection_id=c.id AND r.position=1
          WHERE c.deleted_at IS NULL ORDER BY c.sort_key COLLATE BINARY,c.id";
        let rows = self
            .connection
            .query_all_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
            .await?;
        rows.into_iter()
            .map(|row| {
                let count: i64 = row.try_get("", "item_count")?;
                let cover: Option<String> = row.try_get("", "cover_id")?;
                Ok(CollectionSummary {
                    collection: entities::collection::Model::from_query_result(&row, "")?
                        .domain()?,
                    count: u64::try_from(count)
                        .map_err(|_| crate::StorageError::Integrity("negative collection count"))?,
                    cover: cover
                        .map(|s| s.parse().map_err(crate::StorageError::from))
                        .transpose()?,
                })
            })
            .collect()
    }

    /// Historical memberships are suggestions, never effective associations.
    pub async fn previous_sticker_collection(&self, id: StickerId) -> Result<Option<Collection>> {
        let sql = "SELECT c.* FROM collections c JOIN collection_items ci ON ci.collection_id=c.id WHERE ci.sticker_id=? AND ci.present=1 AND c.deleted_at IS NULL";
        entities::collection::Entity::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                [id.to_string().into()],
            ))
            .one(&self.connection)
            .await?
            .map(|row| row.domain())
            .transpose()
    }
    pub async fn sticker_collection(&self, id: StickerId) -> Result<Option<Collection>> {
        let sql = "SELECT c.* FROM collections c JOIN collection_items r ON r.collection_id=c.id
            JOIN stickers s ON s.id=r.sticker_id WHERE s.id=? AND s.deleted_at IS NULL
            AND c.deleted_at IS NULL AND r.present=1 AND r.collection_generation=c.generation
            AND r.sticker_generation=s.generation";
        entities::collection::Entity::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                [id.to_string().into()],
            ))
            .one(&self.connection)
            .await?
            .map(|row| row.domain())
            .transpose()
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
        entities::collection_item::Entity::find_by_id(sticker.to_string())
            .filter(entities::collection_item::Column::CollectionId.eq(collection.to_string()))
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
}
