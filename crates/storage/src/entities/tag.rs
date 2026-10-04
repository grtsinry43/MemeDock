super::row!("tags", {
    #[sea_orm(primary_key, auto_increment = false)] id: String,
    name: String, normalized_name: String, generation: i64, revision: i64,
    created_at: i64, updated_at: i64, deleted_at: Option<i64>,
});
