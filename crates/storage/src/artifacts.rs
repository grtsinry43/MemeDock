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
    pub output_hash: ContentHash,
    pub recipe: String,
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
        if v.recipe.is_empty()
            || v.recipe.len() > 256
            || !matches!(v.state.as_str(), "ready" | "deleting")
        {
            return Err(StorageError::Integrity("invalid artifact state"));
        }
        let source_hash = v.source_hash.parse()?;
        let output_hash = v.output_hash.parse()?;
        if v.recipe == "original-v1" && source_hash != output_hash {
            return Err(StorageError::Integrity("original artifact hash mismatch"));
        }
        Ok(Self {
            id: v.id.parse()?,
            source_hash,
            output_hash,
            recipe: v.recipe,
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
    /// Pending references protect both sides of a platform handoff across crashes.
    pub async fn protect_clipboard(&self, id: OperationId, artifact_id: OperationId) -> Result<()> {
        use crate::entities::clipboard_reference as reference;
        let tx = self.begin_write().await?;
        let artifact = row::Entity::find_by_id(artifact_id.to_string())
            .one(&tx.inner)
            .await?
            .ok_or(StorageError::InvalidInput("clipboard artifact missing"))?;
        if artifact.state != "ready" {
            return Err(StorageError::InvalidInput("clipboard artifact unavailable"));
        }
        reference::ActiveModel {
            id: Set(id.to_string()),
            artifact_id: Set(artifact_id.to_string()),
            state: Set("pending".into()),
        }
        .insert(&tx.inner)
        .await?;
        tx.commit().await
    }
    pub async fn artifact_clipboard_protected(&self, artifact_id: OperationId) -> Result<bool> {
        use crate::entities::clipboard_reference as reference;
        Ok(reference::Entity::find()
            .filter(reference::Column::ArtifactId.eq(artifact_id.to_string()))
            .one(&self.connection)
            .await?
            .is_some())
    }
    /// Call only after observing the platform clipboard. None means no owned URI.
    pub async fn reconcile_clipboard(&self, observed: Option<OperationId>) -> Result<()> {
        use crate::entities::clipboard_reference as reference;
        let tx = self.begin_write().await?;
        if let Some(id) = observed {
            let record = reference::Entity::find_by_id(id.to_string())
                .one(&tx.inner)
                .await?
                .ok_or(StorageError::InvalidInput("unknown clipboard reference"))?;
            reference::Entity::delete_many()
                .filter(reference::Column::Id.ne(id.to_string()))
                .exec(&tx.inner)
                .await?;
            let mut model: reference::ActiveModel = record.into();
            model.state = Set("current".into());
            model.update(&tx.inner).await?;
        } else {
            reference::Entity::delete_many().exec(&tx.inner).await?;
        }
        tx.commit().await
    }
    pub async fn abort_clipboard(&self, id: OperationId) -> Result<()> {
        use crate::entities::clipboard_reference as reference;
        reference::Entity::delete_many()
            .filter(reference::Column::Id.eq(id.to_string()))
            .filter(reference::Column::State.eq("pending"))
            .exec(&self.connection)
            .await?;
        Ok(())
    }
    pub async fn artifact_exists(&self, id: OperationId) -> Result<bool> {
        Ok(row::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .is_some())
    }
    pub async fn artifact_for(&self, hash: ContentHash) -> Result<Option<ArtifactRecord>> {
        self.artifact_for_recipe(hash, "original-v1").await
    }
    pub async fn artifact_by_id(&self, id: OperationId) -> Result<Option<ArtifactRecord>> {
        row::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .map(TryInto::try_into)
            .transpose()
    }
    pub async fn artifact_for_recipe(
        &self,
        hash: ContentHash,
        recipe: &str,
    ) -> Result<Option<ArtifactRecord>> {
        row::Entity::find()
            .filter(row::Column::SourceHash.eq(hash.to_string()))
            .filter(row::Column::Recipe.eq(recipe))
            .filter(row::Column::State.eq("ready"))
            .one(&self.connection)
            .await?
            .map(TryInto::try_into)
            .transpose()
    }
    pub async fn save_artifact(&self, record: &ArtifactRecord) -> Result<()> {
        if record.recipe == "original-v1" && record.source_hash != record.output_hash {
            return Err(StorageError::InvalidInput(
                "original artifact hash mismatch",
            ));
        }
        let model = row::ActiveModel {
            id: Set(record.id.to_string()),
            source_hash: Set(record.source_hash.to_string()),
            output_hash: Set(record.output_hash.to_string()),
            recipe: Set(record.recipe.clone()),
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
