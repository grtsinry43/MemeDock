super::row!("local_assets", {
    #[sea_orm(primary_key, auto_increment = false)] hash: String,
    blob_status: String, thumb_status: String,
    last_error: Option<String>, verified_at: Option<i64>,
});
