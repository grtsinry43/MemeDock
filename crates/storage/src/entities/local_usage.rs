super::row!("local_usage", {
    #[sea_orm(primary_key, auto_increment = false)] sticker_id: String,
    last_used_at: i64, use_count: i64,
});
