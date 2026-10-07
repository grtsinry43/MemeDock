//! The sole current portable format. No historical format conversion.
use crate::{
    asset::Asset,
    collection::Collection,
    error::DomainError,
    identity::ContentHash,
    lifecycle::{EntityId, Tombstone},
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    tag::Tag,
    version::TimestampMs,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveManifest {
    pub format_version: u32,
    pub created_at: TimestampMs,
    pub metadata_hash: ContentHash,
    pub metadata_bytes: u64,
    pub originals: Vec<ArchiveOriginal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveOriginal {
    pub hash: ContentHash,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveData {
    pub assets: Vec<Asset>,
    pub stickers: Vec<Sticker>,
    pub collections: Vec<Collection>,
    pub tags: Vec<Tag>,
    pub collection_items: Vec<CollectionItem>,
    pub sticker_tags: Vec<StickerTag>,
    pub tombstones: Vec<Tombstone>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreMode {
    Merge,
    Replace,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreSummary {
    pub stickers: u64,
    pub deleted_stickers: u64,
    pub collections: u64,
    pub tags: u64,
    pub original_bytes: u64,
    pub added: u64,
    pub preserved: u64,
    pub skipped_relations: u64,
}

impl ArchiveData {
    pub fn validate(&self) -> Result<(), DomainError> {
        let assets = unique(self.assets.iter().map(|v| (v.hash(), v)))?;
        let stickers = unique(self.stickers.iter().map(|v| (v.id(), v)))?;
        let collections = unique(self.collections.iter().map(|v| (v.id(), v)))?;
        let tags = unique(self.tags.iter().map(|v| (v.id(), v)))?;
        unique(self.collection_items.iter().map(|v| (v.sticker_id(), v)))?;
        unique(
            self.sticker_tags
                .iter()
                .map(|v| ((v.sticker_id(), v.tag_id()), v)),
        )?;
        unique(self.tombstones.iter().map(|v| (entity_key(v.entity()), v)))?;
        for sticker in &self.stickers {
            if !assets.contains_key(&sticker.id().content_hash()) {
                return Err(DomainError::InvalidValue("archive sticker has no original"));
            }
        }
        if assets.len() != stickers.len() {
            return Err(DomainError::InvalidValue(
                "archive has an unreferenced original",
            ));
        }
        for tombstone in &self.tombstones {
            let life = match tombstone.entity() {
                EntityId::Sticker(id) => stickers.get(&id).map(|v| v.lifecycle()),
                EntityId::Collection(id) => collections.get(&id).map(|v| v.lifecycle()),
                EntityId::Tag(id) => tags.get(&id).map(|v| v.lifecycle()),
            };
            if life.is_some_and(|life| {
                tombstone.generation() > life.generation()
                    || (tombstone.generation() == life.generation()
                        && (life.is_active() || tombstone.revision() > life.revision()))
            }) {
                return Err(DomainError::InvalidValue(
                    "archive tombstone contradicts entity",
                ));
            }
        }
        for item in &self.collection_items {
            let collection = collections
                .get(&item.collection_id())
                .ok_or(DomainError::IdentityMismatch)?;
            let sticker = stickers
                .get(&item.sticker_id())
                .ok_or(DomainError::IdentityMismatch)?;
            if item.collection_generation() > collection.lifecycle().generation()
                || item.sticker_generation() > sticker.lifecycle().generation()
            {
                return Err(DomainError::StaleGeneration);
            }
        }
        for item in &self.sticker_tags {
            let sticker = stickers
                .get(&item.sticker_id())
                .ok_or(DomainError::IdentityMismatch)?;
            let tag = tags
                .get(&item.tag_id())
                .ok_or(DomainError::IdentityMismatch)?;
            if item.sticker_generation() > sticker.lifecycle().generation()
                || item.tag_generation() > tag.lifecycle().generation()
            {
                return Err(DomainError::StaleGeneration);
            }
        }
        Ok(())
    }
    pub fn summary(&self) -> Result<RestoreSummary, DomainError> {
        let original_bytes = self.assets.iter().try_fold(0u64, |total, asset| {
            total
                .checked_add(asset.byte_size().get() as u64)
                .ok_or(DomainError::InvalidValue("archive size overflow"))
        })?;
        Ok(RestoreSummary {
            stickers: self.stickers.len() as u64,
            deleted_stickers: self
                .stickers
                .iter()
                .filter(|v| !v.lifecycle().is_active())
                .count() as u64,
            collections: self.collections.len() as u64,
            tags: self.tags.len() as u64,
            original_bytes,
            ..RestoreSummary::default()
        })
    }
}

pub fn entity_key(id: EntityId) -> String {
    match id {
        EntityId::Sticker(v) => format!("sticker:{v}"),
        EntityId::Collection(v) => format!("collection:{v}"),
        EntityId::Tag(v) => format!("tag:{v}"),
    }
}

fn unique<K: Ord, V>(items: impl Iterator<Item = (K, V)>) -> Result<BTreeMap<K, V>, DomainError> {
    let mut result = BTreeMap::new();
    for (key, value) in items {
        if result.insert(key, value).is_some() {
            return Err(DomainError::InvalidValue("duplicate archive identity"));
        }
    }
    Ok(result)
}
