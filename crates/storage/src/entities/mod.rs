// Rows deliberately mirror the version-1 domain serialization field names.
// Mapping is centralized; no row type is exported from storage.
macro_rules! row {
    ($table:literal, { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        use sea_orm::entity::prelude::*;
        #[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
        #[sea_orm(table_name = $table)]
        pub struct Model { $($(#[$attr])* pub $field: $ty),* }
        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}
        impl ActiveModelBehavior for ActiveModel {}
    };
}
pub(crate) use row;
pub mod asset;
pub mod collection;
pub mod collection_item;
pub mod library_metadata;
pub mod local_asset;
pub mod local_change;
pub mod local_usage;
pub mod sticker;
pub mod sticker_tag;
pub mod tag;
pub mod tombstone;
