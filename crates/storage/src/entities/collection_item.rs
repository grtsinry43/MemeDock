super::row!("collection_items", {
    #[sea_orm(primary_key, auto_increment = false)] collection_id: String,
    #[sea_orm(primary_key, auto_increment = false)] sticker_id: String,
    collection_generation: i64, sticker_generation: i64, present: bool,
    sort_key: String, revision: i64, updated_at: i64,
});
