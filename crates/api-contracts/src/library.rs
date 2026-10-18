use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryQuery {
    pub q: String,
    pub collection_id: Option<String>,
    pub tag_id: Option<String>,
    pub starred: bool,
    pub deleted: bool,
    pub page: u32,
    pub limit: u32,
}
impl Default for LibraryQuery {
    fn default() -> Self {
        Self {
            q: String::new(),
            collection_id: None,
            tag_id: None,
            starred: false,
            deleted: false,
            page: 1,
            limit: 36,
        }
    }
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct FacetQuery {
    pub q: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryAsset {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub animated: bool,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub byte_size: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibrarySticker {
    pub id: String,
    pub title: String,
    pub original_name: String,
    pub starred: bool,
    pub deleted: bool,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub created_at: i64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub updated_at: i64,
    pub asset: LibraryAsset,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryPage {
    pub items: Vec<LibrarySticker>,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryFacet {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryFacets {
    pub items: Vec<LibraryFacet>,
    pub has_more: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct LibraryDetail {
    pub sticker: LibrarySticker,
    pub note: String,
    pub collection: Option<LibraryFacet>,
    pub tags: Vec<LibraryFacet>,
}
