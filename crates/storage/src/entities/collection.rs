super::row!("collections", {
    #[sea_orm(primary_key, auto_increment = false)] id: String,
    name: String, sort_key: String, generation: i64, revision: i64,
    created_at: i64, updated_at: i64, deleted_at: Option<i64>,
});
