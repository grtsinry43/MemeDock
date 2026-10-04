super::row!("assets", {
    #[sea_orm(primary_key, auto_increment = false)] hash: String,
    byte_size: i64, mime: String, width: i64, height: i64,
    animated: bool, created_at: i64,
});
