use crate::{LibraryDatabase, Result, entities};
use memedock_domain::{
    identity::{StickerId, TagId},
    relation::StickerTag,
    tag::Tag,
};
use sea_orm::{ColumnTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, Statement};
impl LibraryDatabase {
    pub async fn tag(&self, id: TagId) -> Result<Option<Tag>> {
        entities::tag::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn tags(&self, deleted: bool) -> Result<Vec<Tag>> {
        let col = entities::tag::Column::DeletedAt;
        entities::tag::Entity::find()
            .filter(if deleted {
                col.is_not_null()
            } else {
                col.is_null()
            })
            .order_by_asc(entities::tag::Column::NormalizedName)
            .order_by_asc(entities::tag::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|r| r.domain())
            .collect()
    }
    pub async fn sticker_tags(&self, id: StickerId) -> Result<Vec<Tag>> {
        let sql = "SELECT t.* FROM tags t JOIN sticker_tags st ON st.tag_id=t.id JOIN stickers s ON s.id=st.sticker_id WHERE s.id=? AND s.deleted_at IS NULL AND t.deleted_at IS NULL AND st.present=1 AND st.sticker_generation=s.generation AND st.tag_generation=t.generation ORDER BY t.normalized_name,t.id";
        entities::tag::Entity::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                [id.to_string().into()],
            ))
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|r| r.domain())
            .collect()
    }
    pub async fn sticker_tag(&self, sticker: StickerId, tag: TagId) -> Result<Option<StickerTag>> {
        entities::sticker_tag::Entity::find_by_id((sticker.to_string(), tag.to_string()))
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
}
