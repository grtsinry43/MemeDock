super::row!("entity_tombstones", {
    #[sea_orm(primary_key, auto_increment = false)] entity_kind: String,
    #[sea_orm(primary_key, auto_increment = false)] entity_id: String,
    generation: i64, revision: i64, deleted_at: i64,
});
