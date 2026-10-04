super::row!("sticker_tags", {
    #[sea_orm(primary_key, auto_increment = false)] sticker_id: String,
    #[sea_orm(primary_key, auto_increment = false)] tag_id: String,
    sticker_generation: i64, tag_generation: i64, present: bool,
    revision: i64, updated_at: i64,
});
