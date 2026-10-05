//! Local output records. Core serializes registration, acquisition and cleanup.
use crate::{LibraryDatabase, Result, StorageError, entities::export_artifact as row};
use memedock_domain::{
    asset::ImageFormat,
    identity::{ContentHash, OperationId},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

#[derive(Clone, Debug)]
pub struct ArtifactRecord {
    pub id: OperationId,
    pub source_hash: ContentHash,
    pub format: ImageFormat,
    pub byte_size: u64,
    pub animated: bool,
    pub retained_until: i64,
    pub deleting: bool,
}
impl TryFrom<row::Model> for ArtifactRecord {
    type Error = StorageError;
    fn try_from(v: row::Model) -> Result<Self> {
        let format = match v.mime.as_str() {
            "image/png" => ImageFormat::Png,
            "image/jpeg" => ImageFormat::Jpeg,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::WebP,
            _ => return Err(StorageError::Integrity("invalid artifact format")),
        };
        if v.recipe != "original-v1" || !matches!(v.state.as_str(), "ready" | "deleting") {
            return Err(StorageError::Integrity("invalid artifact state"));
        }
        Ok(Self {
            id: v.id.parse()?,
            source_hash: v.source_hash.parse()?,
            format,
            byte_size: u64::try_from(v.byte_size)
                .map_err(|_| StorageError::Integrity("negative artifact size"))?,
            animated: v.animated,
            retained_until: v.retained_until,
            deleting: v.state == "deleting",
        })
    }
}
impl LibraryDatabase {
    pub async fn artifact_exists(&self, id: OperationId) -> Result<bool> {
        Ok(row::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .is_some())
    }
    pub async fn artifact_for(&self, hash: ContentHash) -> Result<Option<ArtifactRecord>> {
        row::Entity::find()
            .filter(row::Column::SourceHash.eq(hash.to_string()))
            .filter(row::Column::State.eq("ready"))
            .one(&self.connection)
            .await?
            .map(TryInto::try_into)
            .transpose()
    }
    pub async fn save_artifact(&self, record: &ArtifactRecord) -> Result<()> {
        let model = row::ActiveModel {
            id: Set(record.id.to_string()),
            source_hash: Set(record.source_hash.to_string()),
            recipe: Set("original-v1".into()),
            mime: Set(record.format.mime().into()),
            byte_size: Set(i64::try_from(record.byte_size)
                .map_err(|_| StorageError::InvalidInput("artifact size overflow"))?),
            animated: Set(record.animated),
            retained_until: Set(record.retained_until),
            state: Set(if record.deleting { "deleting" } else { "ready" }.into()),
        };
        if row::Entity::find_by_id(record.id.to_string())
            .one(&self.connection)
            .await?
            .is_some()
        {
            model.reset_all().update(&self.connection).await?;
        } else {
            model.insert(&self.connection).await?;
        }
        Ok(())
    }
    pub async fn remove_artifact(&self, id: OperationId) -> Result<()> {
        row::Entity::delete_by_id(id.to_string())
            .exec(&self.connection)
            .await?;
        Ok(())
    }
    /// Bounded keyset scan, including interrupted deletions.
    pub async fn artifact_page(&self, after: Option<OperationId>) -> Result<Vec<ArtifactRecord>> {
        let mut query = row::Entity::find();
        if let Some(after) = after {
            query = query.filter(row::Column::Id.gt(after.to_string()));
        }
        query
            .order_by_asc(row::Column::Id)
            .limit(100)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
}
