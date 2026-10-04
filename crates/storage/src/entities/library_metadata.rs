super::row!("library_metadata", {
    #[sea_orm(primary_key, auto_increment = false)] singleton: i64,
    library_id: String, device_id: String,
});
