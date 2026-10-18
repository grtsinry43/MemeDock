//! Bounded, read-only views of the authoritative catalog. ORM rows stay private.
use super::{ServerDatabase, sync::decode};
use crate::{Result, StorageError};
use memedock_domain::{
    asset::Asset,
    collection::Collection,
    identity::{CollectionId, StickerId, TagId},
    sticker::Sticker,
    tag::Tag,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value};

#[derive(Default)]
pub struct BrowseFilter {
    pub query: String,
    pub collection: Option<CollectionId>,
    pub tag: Option<TagId>,
    pub starred: bool,
    pub deleted: bool,
    pub page: u32,
    pub limit: u32,
}
pub struct BrowseSticker {
    pub sticker: Sticker,
    pub asset: Asset,
}
pub struct BrowsePage {
    pub items: Vec<BrowseSticker>,
    pub total: u64,
}
pub struct BrowseDetail {
    pub item: BrowseSticker,
    pub collection: Option<Collection>,
    pub tags: Vec<Tag>,
}
pub struct NamedFacet {
    pub id: String,
    pub name: String,
}
pub struct FacetPage {
    pub items: Vec<NamedFacet>,
    pub has_more: bool,
}

const COLLECTION: &str = "FROM server_sync_entities r JOIN server_sync_entities c ON c.kind='collection' AND c.id=r.parent_id WHERE r.kind='collection_item' AND r.id=s.id AND json_extract(r.payload,'$.present')=1 AND json_extract(c.payload,'$.deleted_at') IS NULL AND json_extract(c.payload,'$.generation')=json_extract(r.payload,'$.collection_generation') AND json_extract(s.payload,'$.generation')=json_extract(r.payload,'$.sticker_generation')";
const TAG: &str = "FROM server_sync_entities r JOIN server_sync_entities t ON t.kind='tag' AND t.id=json_extract(r.payload,'$.tag_id') WHERE r.kind='sticker_tag' AND json_extract(r.payload,'$.sticker_id')=s.id AND json_extract(r.payload,'$.present')=1 AND json_extract(t.payload,'$.deleted_at') IS NULL AND json_extract(t.payload,'$.generation')=json_extract(r.payload,'$.tag_generation') AND json_extract(s.payload,'$.generation')=json_extract(r.payload,'$.sticker_generation')";

pub(super) fn page_bounds(page: u32, limit: u32, maximum: u32) -> Result<()> {
    if !(1..=10_000).contains(&page) || !(1..=maximum).contains(&limit) {
        return Err(StorageError::InvalidInput("page bounds"));
    }
    Ok(())
}
impl ServerDatabase {
    pub async fn browse_stickers(&self, filter: &BrowseFilter) -> Result<BrowsePage> {
        page_bounds(filter.page, filter.limit, 72)?;
        if filter.query.chars().count() > 256 {
            return Err(StorageError::InvalidInput("search too long"));
        }
        let mut condition = format!(
            "s.kind='sticker' AND json_extract(s.payload,'$.purged_at') IS NULL AND json_extract(s.payload,'$.deleted_at') IS {}NULL",
            if filter.deleted { "NOT " } else { "" }
        );
        let mut values: Vec<Value> = Vec::new();
        if !filter.query.trim().is_empty() {
            condition.push_str(&format!(" AND (instr(lower(json_extract(s.payload,'$.title')),lower(?))>0 OR instr(lower(json_extract(s.payload,'$.note')),lower(?))>0 OR instr(lower(json_extract(s.payload,'$.original_name')),lower(?))>0 OR EXISTS(SELECT 1 {COLLECTION} AND instr(lower(json_extract(c.payload,'$.name')),lower(?))>0) OR EXISTS(SELECT 1 {TAG} AND instr(lower(json_extract(t.payload,'$.name')),lower(?))>0))"));
            values.extend((0..5).map(|_| Value::from(filter.query.trim().to_owned())));
        }
        if let Some(id) = filter.collection {
            condition.push_str(&format!(" AND EXISTS(SELECT 1 {COLLECTION} AND c.id=?)"));
            values.push(id.to_string().into());
        }
        if let Some(id) = filter.tag {
            condition.push_str(&format!(" AND EXISTS(SELECT 1 {TAG} AND t.id=?)"));
            values.push(id.to_string().into());
        }
        if filter.starred {
            condition.push_str(" AND json_extract(s.payload,'$.starred')=1");
        }
        let tx = self.connection.begin().await?;
        let total = tx
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!("SELECT count(*) AS n FROM server_sync_entities s WHERE {condition}"),
                values.clone(),
            ))
            .await?
            .ok_or(StorageError::Integrity("catalog count"))?
            .try_get::<i64>("", "n")? as u64;
        values.push(i64::from(filter.limit).into());
        values.push((i64::from(filter.page - 1) * i64::from(filter.limit)).into());
        let rows = tx.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite, format!("SELECT s.payload AS sticker,a.payload AS asset FROM server_sync_entities s JOIN server_assets a ON a.hash=s.id WHERE {condition} ORDER BY json_extract(s.payload,'$.created_at') DESC,s.id LIMIT ? OFFSET ?"), values)).await?;
        let items = rows
            .into_iter()
            .map(|r| {
                Ok(BrowseSticker {
                    sticker: decode(&r.try_get::<String>("", "sticker")?)?,
                    asset: decode(&r.try_get::<String>("", "asset")?)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(BrowsePage { items, total })
    }

    pub async fn browse_detail(&self, id: StickerId) -> Result<Option<BrowseDetail>> {
        let tx = self.connection.begin().await?;
        let Some(row) = tx.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite, "SELECT s.payload AS sticker,a.payload AS asset FROM server_sync_entities s JOIN server_assets a ON a.hash=s.id WHERE s.kind='sticker' AND s.id=?", [id.to_string().into()])).await? else { return Ok(None) };
        let item = BrowseSticker {
            sticker: decode(&row.try_get::<String>("", "sticker")?)?,
            asset: decode(&row.try_get::<String>("", "asset")?)?,
        };
        // Keep the relation joins in a correlated subquery, returning owned metadata.
        let collection = tx.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite, format!("SELECT (SELECT c.payload {COLLECTION} LIMIT 1) AS payload FROM server_sync_entities s WHERE s.kind='sticker' AND s.id=? AND json_extract(s.payload,'$.deleted_at') IS NULL"), [id.to_string().into()])).await?.map(|r| r.try_get::<Option<String>>("", "payload")).transpose()?.flatten().map(|v| decode(&v)).transpose()?;
        let rows = tx.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite, "SELECT t.payload FROM server_sync_entities s JOIN server_sync_entities r ON json_extract(r.payload,'$.sticker_id')=s.id JOIN server_sync_entities t ON t.kind='tag' AND t.id=json_extract(r.payload,'$.tag_id') WHERE s.kind='sticker' AND s.id=? AND json_extract(s.payload,'$.deleted_at') IS NULL AND r.kind='sticker_tag' AND json_extract(r.payload,'$.present')=1 AND json_extract(t.payload,'$.deleted_at') IS NULL AND json_extract(t.payload,'$.generation')=json_extract(r.payload,'$.tag_generation') AND json_extract(s.payload,'$.generation')=json_extract(r.payload,'$.sticker_generation') ORDER BY json_extract(t.payload,'$.normalized_name'),t.id LIMIT 1001", [id.to_string().into()])).await?;
        if rows.len() > 1000 {
            return Err(StorageError::InvalidInput("too many tags for detail"));
        }
        let tags = rows
            .into_iter()
            .map(|r| decode(&r.try_get::<String>("", "payload")?))
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(Some(BrowseDetail {
            item,
            collection,
            tags,
        }))
    }

    /// Searchable facets: avoid materializing an unbounded tag/collection directory.
    pub async fn browse_facets(&self, collections: bool, query: &str) -> Result<FacetPage> {
        if query.chars().count() > 128 {
            return Err(StorageError::InvalidInput("facet search too long"));
        }
        let rows = self.connection.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite, "SELECT id,json_extract(payload,'$.name') AS name FROM server_sync_entities WHERE kind=? AND json_extract(payload,'$.deleted_at') IS NULL AND instr(lower(json_extract(payload,'$.name')),lower(?))>0 ORDER BY json_extract(payload,'$.name'),id LIMIT 101", [if collections { "collection" } else { "tag" }.into(), query.trim().to_owned().into()])).await?;
        let has_more = rows.len() > 100;
        let items = rows
            .into_iter()
            .take(100)
            .map(|r| {
                Ok(NamedFacet {
                    id: r.try_get("", "id")?,
                    name: r.try_get("", "name")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(FacetPage { items, has_more })
    }
}
