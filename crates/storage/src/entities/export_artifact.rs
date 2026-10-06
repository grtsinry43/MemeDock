super::row!("export_artifacts", {
    #[sea_orm(primary_key, auto_increment = false)] id: String,
    source_hash: String, output_hash: String, recipe: String, mime: String, byte_size: i64,
    animated: bool, retained_until: i64, state: String,
});
