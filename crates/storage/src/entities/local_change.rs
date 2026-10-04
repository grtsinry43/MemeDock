super::row!("local_changes", {
    #[sea_orm(primary_key)] local_order: i64,
    op_id: String, schema_version: i64, payload_json: String, status: String,
    attempts: i64, retry_after: Option<i64>, accepted_revision: Option<i64>,
    last_error: Option<String>, created_at: i64,
});
