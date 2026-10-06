super::row!("clipboard_references", {
    #[sea_orm(primary_key, auto_increment = false)] id: String,
    artifact_id: String, state: String,
});
