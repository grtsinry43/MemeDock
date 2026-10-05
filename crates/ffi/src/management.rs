use crate::*;
use memedock_domain::{
    change::{FieldPatch, StickerPatch},
    tag::Name,
    version::{Generation, Revision},
};
use std::sync::Arc;

#[derive(Clone, Debug, uniffi::Record)]
pub struct EntityReference {
    pub id: String,
    pub generation: i64,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct StickerEdit {
    pub title: Option<String>,
    pub note: Option<String>,
    pub starred: Option<bool>,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct RestoreSuggestions {
    pub collections: Vec<CollectionMetadata>,
    pub tags: Vec<TagMetadata>,
}

#[uniffi::export]
impl LibraryHandle {
    pub fn delete_sticker(&self, target: EntityReference) -> Result<Arc<StickerMutationTask>> {
        Ok(StickerMutationTask::new(self.inner.delete_sticker(
            target.id.parse()?,
            Generation::new(target.generation)?,
        )?))
    }
    pub fn restore_sticker(
        &self,
        target: EntityReference,
        deleted_revision: i64,
    ) -> Result<Arc<StickerMutationTask>> {
        Ok(StickerMutationTask::new(self.inner.restore_sticker(
            target.id.parse()?,
            Generation::new(target.generation)?,
            Revision::new(deleted_revision)?,
        )?))
    }

    pub fn rename_collection(
        &self,
        target: EntityReference,
        name: String,
    ) -> Result<Arc<CollectionMutationTask>> {
        Ok(CollectionMutationTask::new(self.inner.rename_collection(
            target.id.parse()?,
            Generation::new(target.generation)?,
            Name::new(name)?,
        )?))
    }

    pub fn delete_collection(
        &self,
        target: EntityReference,
    ) -> Result<Arc<CollectionMutationTask>> {
        Ok(CollectionMutationTask::new(self.inner.delete_collection(
            target.id.parse()?,
            Generation::new(target.generation)?,
        )?))
    }
    pub fn restore_collection(
        &self,
        target: EntityReference,
        deleted_revision: i64,
    ) -> Result<Arc<CollectionMutationTask>> {
        Ok(CollectionMutationTask::new(self.inner.restore_collection(
            target.id.parse()?,
            Generation::new(target.generation)?,
            Revision::new(deleted_revision)?,
        )?))
    }

    pub fn rename_tag(
        &self,
        target: EntityReference,
        name: String,
    ) -> Result<Arc<TagMutationTask>> {
        Ok(TagMutationTask::new(self.inner.rename_tag(
            target.id.parse()?,
            Generation::new(target.generation)?,
            Name::new(name)?,
        )?))
    }

    pub fn delete_tag(&self, target: EntityReference) -> Result<Arc<TagMutationTask>> {
        Ok(TagMutationTask::new(self.inner.delete_tag(
            target.id.parse()?,
            Generation::new(target.generation)?,
        )?))
    }
    pub fn restore_tag(
        &self,
        target: EntityReference,
        deleted_revision: i64,
    ) -> Result<Arc<TagMutationTask>> {
        Ok(TagMutationTask::new(self.inner.restore_tag(
            target.id.parse()?,
            Generation::new(target.generation)?,
            Revision::new(deleted_revision)?,
        )?))
    }
    pub fn patch_sticker(
        &self,
        target: EntityReference,
        edit: StickerEdit,
    ) -> Result<Arc<StickerMutationTask>> {
        let patch = StickerPatch::new(
            edit.title.map_or(FieldPatch::Missing, FieldPatch::Set),
            edit.note.map_or(FieldPatch::Missing, FieldPatch::Set),
            edit.starred.map_or(FieldPatch::Missing, FieldPatch::Set),
        )?;
        Ok(StickerMutationTask::new(self.inner.patch_sticker(
            target.id.parse()?,
            Generation::new(target.generation)?,
            patch,
        )?))
    }
    pub fn set_sticker_relations(
        &self,
        target: EntityReference,
        collections: Option<Vec<EntityReference>>,
        tags: Option<Vec<EntityReference>>,
    ) -> Result<Arc<MutationTask>> {
        let collections = collections
            .map(|refs| {
                refs.into_iter()
                    .map(|r| Ok((r.id.parse()?, Generation::new(r.generation)?)))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let tags = tags
            .map(|refs| {
                refs.into_iter()
                    .map(|r| Ok((r.id.parse()?, Generation::new(r.generation)?)))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(MutationTask::new(self.inner.set_sticker_relations(
            target.id.parse()?,
            Generation::new(target.generation)?,
            collections,
            tags,
        )?))
    }
    pub fn create_collection(
        &self,
        name: String,
        before: Option<String>,
    ) -> Result<Arc<CollectionMutationTask>> {
        Ok(CollectionMutationTask::new(self.inner.create_collection(
            Name::new(name)?,
            before.map(|id| id.parse()).transpose()?,
        )?))
    }
    pub fn move_collection(
        &self,
        target: EntityReference,
        before: Option<String>,
    ) -> Result<Arc<CollectionMutationTask>> {
        Ok(CollectionMutationTask::new(self.inner.move_collection(
            target.id.parse()?,
            Generation::new(target.generation)?,
            before.map(|id| id.parse()).transpose()?,
        )?))
    }
    pub fn move_collection_item(
        &self,
        collection: EntityReference,
        sticker: EntityReference,
        before: Option<String>,
    ) -> Result<Arc<MutationTask>> {
        Ok(MutationTask::new(self.inner.move_collection_item(
            collection.id.parse()?,
            Generation::new(collection.generation)?,
            sticker.id.parse()?,
            Generation::new(sticker.generation)?,
            before.map(|id| id.parse()).transpose()?,
        )?))
    }
    pub fn create_tag(&self, name: String) -> Result<Arc<TagMutationTask>> {
        Ok(TagMutationTask::new(
            self.inner.create_tag(Name::new(name)?)?,
        ))
    }
    pub fn restore_suggestions(&self, id: String) -> Result<Arc<RestoreSuggestionsTask>> {
        Ok(RestoreSuggestionsTask::new(
            self.inner.restore_suggestions(id.parse()?)?,
        ))
    }
}
