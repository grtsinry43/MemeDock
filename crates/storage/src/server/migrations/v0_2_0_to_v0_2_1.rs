//! One-time protocol normalization, preserving receipts and operation identity.
use crate::Result;
use sea_orm::ConnectionTrait;

pub(in crate::server) async fn apply(db: &impl ConnectionTrait) -> Result<()> {
    db.execute_unprepared(
        "UPDATE server_sync_receipts SET request=json_set(request,'$.protocol_version',2)",
    )
    .await?;
    db.execute_unprepared("PRAGMA user_version=5").await?;
    add_device_deletion(db).await?;
    Ok(())
}

/// Historic operations belong to the server library. Keep their source name
/// independently of the credential row, which administrators may remove.
pub(in crate::server) async fn add_device_deletion(db: &impl ConnectionTrait) -> Result<()> {
    for sql in [
        "CREATE TABLE server_sync_receipts_v021 (device_id TEXT NOT NULL CHECK(length(device_id)=36), op_id TEXT NOT NULL, request TEXT NOT NULL CHECK(json_valid(request)), receipt TEXT NOT NULL CHECK(json_valid(receipt)), received_at INTEGER, device_name TEXT NOT NULL CHECK(length(device_name) BETWEEN 1 AND 80), PRIMARY KEY(device_id,op_id)) STRICT",
        "INSERT INTO server_sync_receipts_v021(device_id,op_id,request,receipt,received_at,device_name) SELECT r.device_id,r.op_id,r.request,r.receipt,r.received_at,d.name FROM server_sync_receipts r LEFT JOIN server_devices d ON d.id=r.device_id",
        "DROP TABLE server_sync_receipts",
        "ALTER TABLE server_sync_receipts_v021 RENAME TO server_sync_receipts",
        "CREATE INDEX server_receipt_time ON server_sync_receipts(received_at DESC,device_id,op_id)",
        "CREATE INDEX server_receipt_device_time ON server_sync_receipts(device_id,received_at DESC,op_id)",
        "PRAGMA user_version=6",
    ] {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}
