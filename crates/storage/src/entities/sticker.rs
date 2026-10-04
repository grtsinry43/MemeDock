super::row!("stickers", {
    #[sea_orm(primary_key, auto_increment = false)] id: String,
    title: String, original_name: String, note: String, starred: bool,
    generation: i64, revision: i64, created_at: i64, updated_at: i64,
    deleted_at: Option<i64>, normalized_search_text: String,
});
