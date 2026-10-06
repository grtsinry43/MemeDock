use crate::{LibraryDatabase, Result, StorageError, entities};
use memedock_domain::{
    asset::Asset,
    identity::{CollectionId, ContentHash, StickerId, TagId},
    ordering::SortKey,
    sticker::Sticker,
    tag::normalize_text,
};
use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, FromQueryResult, Statement, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StickerSort {
    #[default]
    Recent,
    CollectionOrder,
    LastUsed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StickerQuery {
    pub text: String,
    pub collection: Option<CollectionId>,
    pub tags: Vec<TagId>,
    pub starred: Option<bool>,
    pub deleted: bool,
    pub sort: StickerSort,
}
impl StickerQuery {
    fn normalized(&self) -> Self {
        let mut q = self.clone();
        q.text = normalize_text(&q.text);
        q.tags.sort_unstable();
        q.tags.dedup();
        q
    }
}
/// Owned keyset cursor. Its private fields prevent arbitrary SQL keys and bind it
/// to the normalized query. It is invalidated/reloaded after edits by core.
#[derive(Clone, Debug)]
pub struct PageCursor {
    query: StickerQuery,
    key: CursorKey,
    id: StickerId,
}
#[derive(Clone, Debug)]
enum CursorKey {
    Time(i64),
    Order(SortKey),
}
#[derive(Debug)]
pub struct StickerPage {
    pub stickers: Vec<Sticker>,
    pub next: Option<PageCursor>,
}

const EFFECTIVE_TAG: &str = "st.sticker_id=s.id AND st.present=1 AND st.sticker_generation=s.generation AND st.tag_generation=t.generation AND t.deleted_at IS NULL";

impl LibraryDatabase {
    pub async fn asset(&self, hash: ContentHash) -> Result<Option<Asset>> {
        entities::asset::Entity::find_by_id(hash.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    /// Includes deleted records so core can distinguish reuse from recovery.
    pub async fn sticker(&self, id: StickerId) -> Result<Option<Sticker>> {
        entities::sticker::Entity::find_by_id(id.to_string())
            .one(&self.connection)
            .await?
            .map(|r| r.domain())
            .transpose()
    }
    pub async fn stickers(
        &self,
        query: &StickerQuery,
        page_size: u32,
        cursor: Option<&PageCursor>,
    ) -> Result<StickerPage> {
        if !(1..=200).contains(&page_size) {
            return Err(StorageError::InvalidInput("page size must be 1..=200"));
        }
        let q = query.normalized();
        if q.sort == StickerSort::CollectionOrder && q.collection.is_none() {
            return Err(StorageError::InvalidInput(
                "collection ordering needs a collection",
            ));
        }
        if cursor.is_some_and(|c| c.query != q) {
            return Err(StorageError::InvalidInput(
                "cursor belongs to another query",
            ));
        }
        let mut args: Vec<Value> = Vec::new();
        let mut sql = "SELECT s.* FROM stickers s ".to_owned();
        if q.collection.is_some() {
            sql.push_str("JOIN collection_items ci ON ci.sticker_id=s.id JOIN collections c ON c.id=ci.collection_id ");
        }
        if q.sort == StickerSort::LastUsed {
            sql.push_str("LEFT JOIN local_usage u ON u.sticker_id=s.id ");
        }
        sql.push_str(if q.deleted {
            "WHERE s.deleted_at IS NOT NULL "
        } else {
            "WHERE s.deleted_at IS NULL "
        });
        if let Some(id) = q.collection {
            sql.push_str("AND ci.collection_id=? AND ci.present=1 AND c.deleted_at IS NULL AND ci.collection_generation=c.generation AND ci.sticker_generation=s.generation ");
            args.push(id.to_string().into());
            // A deleted sticker never has effective relationships.
            if q.deleted {
                sql.push_str("AND 0=1 ");
            }
        }
        if let Some(starred) = q.starred {
            sql.push_str("AND s.starred=? ");
            args.push(starred.into());
        }
        for id in &q.tags {
            sql.push_str(&format!("AND EXISTS (SELECT 1 FROM sticker_tags st JOIN tags t ON t.id=st.tag_id WHERE {EFFECTIVE_TAG} AND t.id=?) "));
            args.push(id.to_string().into());
            if q.deleted {
                sql.push_str("AND 0=1 ");
            }
        }
        for token in q.text.split_whitespace() {
            sql.push_str(&format!("AND (instr(s.normalized_search_text,?)>0 OR EXISTS (SELECT 1 FROM sticker_tags st JOIN tags t ON t.id=st.tag_id WHERE {EFFECTIVE_TAG} AND s.deleted_at IS NULL AND instr(t.normalized_name,?)>0)) "));
            args.push(token.into());
            args.push(token.into());
        }
        let (key, ascending) = match q.sort {
            StickerSort::Recent => (
                if q.deleted {
                    "s.deleted_at"
                } else {
                    "s.created_at"
                },
                false,
            ),
            StickerSort::CollectionOrder => ("ci.sort_key COLLATE BINARY", true),
            // Never-used stickers trail every used one, newest created first; the
            // clamp keeps the shifted key inside i64 for any stored timestamp.
            StickerSort::LastUsed => (
                "CASE WHEN u.last_used_at IS NULL THEN max(s.created_at,-4611686018427387904)-4611686018427387904 ELSE u.last_used_at END",
                false,
            ),
        };
        if let Some(c) = cursor {
            let value: Value = match &c.key {
                CursorKey::Time(t) => (*t).into(),
                CursorKey::Order(k) => k.to_string().into(),
            };
            let cmp = if ascending { ">" } else { "<" };
            sql.push_str(&format!(
                "AND ({key} {cmp} ? OR ({key}=? AND s.id {cmp} ?)) "
            ));
            args.extend([value.clone(), value, c.id.to_string().into()]);
        }
        sql.push_str(&format!(
            "ORDER BY {key} {},s.id {} LIMIT ?",
            if ascending { "ASC" } else { "DESC" },
            if ascending { "ASC" } else { "DESC" }
        ));
        args.push(i64::from(page_size + 1).into());
        // Read the page and its cursor key in one SQLite snapshot.
        sql = sql.replacen("SELECT s.*", &format!("SELECT s.*, {key} AS cursor_key"), 1);
        let rows = self
            .connection
            .query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite, sql, args))
            .await?;
        let has_more = rows.len() > page_size as usize;
        let mut stickers = Vec::with_capacity(page_size as usize);
        let mut next = None;
        for row in rows.into_iter().take(page_size as usize) {
            let sticker = entities::sticker::Model::from_query_result(&row, "")?.domain()?;
            let cursor_key = if ascending {
                CursorKey::Order(row.try_get::<String>("", "cursor_key")?.parse()?)
            } else {
                CursorKey::Time(row.try_get("", "cursor_key")?)
            };
            if has_more {
                next = Some(PageCursor {
                    query: q.clone(),
                    key: cursor_key,
                    id: sticker.id(),
                });
            }
            stickers.push(sticker);
        }
        Ok(StickerPage { stickers, next })
    }
}
