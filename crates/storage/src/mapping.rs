use crate::{Result, StorageError, entities};
use memedock_domain::{
    asset::Asset,
    collection::Collection,
    lifecycle::Tombstone,
    local::{LocalAsset, LocalChange, LocalUsage},
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    tag::{Tag, normalize_text},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn convert<A: Serialize, B: DeserializeOwned>(a: A, table: &'static str) -> Result<B> {
    let value =
        serde_json::to_value(a).map_err(|source| StorageError::InvalidData { table, source })?;
    decode(value, table)
}
fn decode<B: DeserializeOwned>(value: Value, table: &'static str) -> Result<B> {
    serde_json::from_value(value).map_err(|source| StorageError::InvalidData { table, source })
}

// Domain serde is a validated, versioned contract. These explicit adapters keep
// database columns independent of protocol JSON while reusing its validation.
macro_rules! adapter {
    ($module:ident, $domain:ty, $table:literal) => {
        impl entities::$module::Model {
            pub(crate) fn domain(self) -> Result<$domain> {
                convert(self, $table)
            }
            pub(crate) fn from_domain(value: &$domain) -> Result<Self> {
                convert(value, $table)
            }
        }
    };
}
adapter!(asset, Asset, "assets");
adapter!(collection, Collection, "collections");
adapter!(tag, Tag, "tags");
adapter!(collection_item, CollectionItem, "collection_items");
adapter!(sticker_tag, StickerTag, "sticker_tags");
adapter!(tombstone, Tombstone, "entity_tombstones");
adapter!(local_asset, LocalAsset, "local_assets");
adapter!(local_usage, LocalUsage, "local_usage");

impl entities::sticker::Model {
    pub(crate) fn domain(self) -> Result<Sticker> {
        convert(self, "stickers")
    }
    pub(crate) fn from_domain(value: &Sticker) -> Result<Self> {
        let mut json = serde_json::to_value(value).map_err(|source| StorageError::InvalidData {
            table: "stickers",
            source,
        })?;
        let search = normalize_text(&format!(
            "{} {} {}",
            value.title(),
            value.original_name(),
            value.note()
        ));
        json.as_object_mut()
            .ok_or(StorageError::Integrity("sticker must be an object"))?
            .insert("normalized_search_text".into(), search.into());
        decode(json, "stickers")
    }
}
impl entities::local_change::Model {
    pub(crate) fn domain(self) -> Result<LocalChange> {
        let operation: Value = serde_json::from_str(&self.payload_json).map_err(|source| {
            StorageError::InvalidData {
                table: "local_changes",
                source,
            }
        })?;
        let mut json = serde_json::to_value(self).map_err(|source| StorageError::InvalidData {
            table: "local_changes",
            source,
        })?;
        let object = json
            .as_object_mut()
            .ok_or(StorageError::Integrity("change row must be an object"))?;
        object.remove("payload_json");
        object.insert("operation".into(), operation);
        decode(json, "local_changes")
    }
    pub(crate) fn from_domain(value: &LocalChange) -> Result<Self> {
        let mut json = serde_json::to_value(value).map_err(|source| StorageError::InvalidData {
            table: "local_changes",
            source,
        })?;
        let object = json
            .as_object_mut()
            .ok_or(StorageError::Integrity("change must be an object"))?;
        let operation = object
            .remove("operation")
            .ok_or(StorageError::Integrity("change operation missing"))?;
        let payload =
            serde_json::to_string(&operation).map_err(|source| StorageError::InvalidData {
                table: "local_changes",
                source,
            })?;
        object.insert("payload_json".into(), payload.into());
        decode(json, "local_changes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memedock_domain::{
        identity::{CollectionId, TagId},
        ordering::SortKey,
        tag::Name,
        version::TimestampMs,
    };

    #[test]
    fn domain_validation_rejects_invalid_uuid_sort_key_and_counters()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let collection = Collection::new(
            CollectionId::new(),
            Name::new("收藏".into())?,
            SortKey::default(),
            TimestampMs::new(0),
        );
        let mut row = entities::collection::Model::from_domain(&collection)?;
        row.id = "00000000-0000-0000-0000-000000000000".into();
        assert!(row.domain().is_err());
        let mut row = entities::collection::Model::from_domain(&collection)?;
        row.sort_key = "81".into();
        assert!(row.domain().is_err());
        let mut row = entities::collection::Model::from_domain(&collection)?;
        row.generation = -1;
        assert!(row.domain().is_err());
        let tag = Tag::new(
            TagId::new(),
            Name::new("ＡＢＣ".into())?,
            TimestampMs::new(0),
        );
        let mut row = entities::tag::Model::from_domain(&tag)?;
        row.normalized_name = "ABC".into();
        assert!(row.domain().is_err());
        Ok(())
    }
}
