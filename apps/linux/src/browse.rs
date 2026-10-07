use memedock_core::{StickerQuery, StickerSort};
use memedock_domain::identity::{CollectionId, TagId};

pub const PAGE_SIZE: u32 = 60;
const PREFETCH_ROWS: u32 = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BrowseFilter {
    #[default]
    Recent,
    All,
    Starred,
    Collection(CollectionId),
    Tag(TagId),
    Trash,
}

pub fn sticker_query(filter: &BrowseFilter, text: &str) -> StickerQuery {
    let mut query = StickerQuery {
        text: text.to_owned(),
        ..StickerQuery::default()
    };
    match filter {
        BrowseFilter::Recent => query.sort = StickerSort::LastUsed,
        BrowseFilter::All => {}
        BrowseFilter::Starred => query.starred = Some(true),
        BrowseFilter::Collection(id) => {
            query.collection = Some(*id);
            query.sort = StickerSort::CollectionOrder;
        }
        BrowseFilter::Tag(id) => query.tags = vec![*id],
        BrowseFilter::Trash => query.deleted = true,
    }
    query
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyCopy {
    Library,
    Starred,
    Collection,
    Tag,
    Trash,
    NoMatches,
}

pub fn empty_copy(filter: &BrowseFilter, text: &str) -> EmptyCopy {
    if !text.trim().is_empty() {
        return EmptyCopy::NoMatches;
    }
    match filter {
        BrowseFilter::Recent | BrowseFilter::All => EmptyCopy::Library,
        BrowseFilter::Starred => EmptyCopy::Starred,
        BrowseFilter::Collection(_) => EmptyCopy::Collection,
        BrowseFilter::Tag(_) => EmptyCopy::Tag,
        BrowseFilter::Trash => EmptyCopy::Trash,
    }
}

pub fn near_end(position: u32, count: u32) -> bool {
    count > 0 && position.saturating_add(PREFETCH_ROWS) >= count
}

#[cfg(test)]
mod tests {
    use super::{BrowseFilter, EmptyCopy, empty_copy, near_end, sticker_query};
    use memedock_core::StickerSort;
    use memedock_domain::identity::{CollectionId, TagId};

    #[test]
    fn filters_keep_text_and_exclude_deleted_stickers() {
        let collection = CollectionId::new();
        let tag = TagId::new();
        let cases = [
            (BrowseFilter::Recent, StickerSort::LastUsed, None, None),
            (BrowseFilter::All, StickerSort::Recent, None, None),
            (BrowseFilter::Starred, StickerSort::Recent, Some(true), None),
            (
                BrowseFilter::Collection(collection),
                StickerSort::CollectionOrder,
                None,
                Some(collection),
            ),
        ];
        for (filter, sort, starred, collection_id) in cases {
            let query = sticker_query(&filter, "  猫猫 ");
            assert_eq!(query.text, "  猫猫 ");
            assert!(!query.deleted);
            assert_eq!(query.sort, sort);
            assert_eq!(query.starred, starred);
            assert_eq!(query.collection, collection_id);
            assert!(query.tags.is_empty());
        }
        let query = sticker_query(&BrowseFilter::Tag(tag), "");
        assert_eq!(query.text, "");
        assert!(!query.deleted);
        assert_eq!(query.sort, StickerSort::Recent);
        assert_eq!(query.tags, vec![tag]);
        assert!(query.collection.is_none());
        assert!(query.starred.is_none());
    }

    #[test]
    fn empty_titles_follow_the_active_filter() {
        assert_eq!(empty_copy(&BrowseFilter::Recent, ""), EmptyCopy::Library);
        assert_eq!(empty_copy(&BrowseFilter::All, " "), EmptyCopy::Library);
        assert_eq!(empty_copy(&BrowseFilter::Starred, ""), EmptyCopy::Starred);
        assert_eq!(
            empty_copy(&BrowseFilter::Collection(CollectionId::new()), ""),
            EmptyCopy::Collection
        );
        assert_eq!(
            empty_copy(&BrowseFilter::Tag(TagId::new()), ""),
            EmptyCopy::Tag
        );
        assert_eq!(empty_copy(&BrowseFilter::All, "猫"), EmptyCopy::NoMatches);
        assert_eq!(empty_copy(&BrowseFilter::Trash, ""), EmptyCopy::Trash);
        assert_eq!(empty_copy(&BrowseFilter::Trash, "猫"), EmptyCopy::NoMatches);
        let trash = sticker_query(&BrowseFilter::Trash, "  猫猫 ");
        assert!(trash.deleted);
        assert_eq!(trash.text, "  猫猫 ");
        assert_eq!(trash.sort, StickerSort::Recent);
        assert!(trash.collection.is_none());
        assert!(trash.starred.is_none());
        assert!(trash.tags.is_empty());
    }

    #[test]
    fn prefetch_starts_within_twelve_rows_of_the_end() {
        assert!(!near_end(0, 0));
        assert!(!near_end(40, 60));
        assert!(near_end(48, 60));
        assert!(near_end(59, 60));
    }
}
