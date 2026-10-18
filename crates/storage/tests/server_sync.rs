use memedock_domain::{
    change::{Operation, OperationKind},
    identity::{DeviceId, OperationId},
    sync::{OperationEnvelope, OperationReceipt, ReceiptOutcome},
    version::{EventSeq, TimestampMs},
};
use memedock_storage::server::ServerDatabase;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn monitoring_upgrade_preserves_receipts_and_rolls_back_partial_ddl() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("server.sqlite");
    let db = ServerDatabase::open(&path).await?;
    db.initialize_admin("admin", "retained-hash", "version")
        .await?;
    let device = DeviceId::new();
    db.create_device(&device.to_string(), "retained device", &"1".repeat(64), 0)
        .await?;
    let tx = db.begin_sync(device).await?;
    let identity = tx.identity;
    let op = OperationEnvelope::new(
        identity.library_id,
        identity.epoch,
        device,
        OperationId::new(),
        TimestampMs::new(0),
        Operation::new(OperationKind::CreateTag {
            tag_id: memedock_domain::identity::TagId::new(),
            name: "historic".parse()?,
        })?,
    );
    let receipt = OperationReceipt {
        device_id: device,
        op_id: op.op_id(),
        outcome: ReceiptOutcome::NoOp {
            canonical_revision: identity.head,
        },
    };
    tx.finish(&op, &receipt, None, TimestampMs::new(123))
        .await?;
    db.close().await?;
    let raw = Database::connect(format!("sqlite://{}?mode=rw", path.display())).await?;
    // Reconstruct the previously implemented development schema 2.
    for sql in [
        "DROP TABLE server_purge_cleanup",
        "DROP INDEX server_receipt_time",
        "DROP INDEX server_receipt_device_time",
        "ALTER TABLE server_sync_receipts DROP COLUMN received_at",
        "DROP TABLE server_device_progress",
        "PRAGMA user_version=2",
        "CREATE TABLE server_device_progress(collision INTEGER)",
    ] {
        raw.execute_unprepared(sql).await?;
    }
    assert!(ServerDatabase::open(&path).await.is_err());
    let columns = raw
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA table_info(server_sync_receipts)".to_owned(),
        ))
        .await?;
    assert!(!columns.iter().any(|r| {
        r.try_get::<String>("", "name")
            .is_ok_and(|n| n == "received_at")
    }));
    let version = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA user_version".to_owned(),
        ))
        .await?
        .ok_or("schema marker")?;
    assert_eq!(version.try_get::<i64>("", "user_version")?, 2);
    raw.execute_unprepared("DROP TABLE server_device_progress")
        .await?;
    raw.close().await?;
    let db = ServerDatabase::open(&path).await?;
    assert_eq!(
        db.admin().await?.ok_or("admin")?.password_hash,
        "retained-hash"
    );
    assert_eq!(
        db.device(&device.to_string()).await?.ok_or("device")?.name,
        "retained device"
    );
    let tx = db.begin_sync(device).await?;
    assert_eq!(tx.identity.epoch, identity.epoch);
    assert_eq!(tx.prior_receipt(&op).await?, Some(receipt));
    drop(tx);
    let logs = db
        .sync_activity(&memedock_storage::server::ActivityFilter {
            device: None,
            result: None,
            page: 1,
            limit: 25,
        })
        .await?;
    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].received_at, None);
    db.close().await?;
    ServerDatabase::open(&path).await?.close().await?;
    Ok(())
}

#[tokio::test]
async fn provisional_database_upgrade_is_atomic_preserves_credentials_and_can_retry() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("server.sqlite");
    let raw = Database::connect(format!("sqlite://{}?mode=rwc", path.display())).await?;
    // Frozen authentication-only format, independent of fresh initialization.
    for sql in [
        "CREATE TABLE server_admin(singleton INTEGER PRIMARY KEY, username TEXT NOT NULL,password_hash TEXT NOT NULL,auth_version TEXT NOT NULL) STRICT",
        "CREATE TABLE server_sessions(id_hash TEXT PRIMARY KEY,payload TEXT NOT NULL,expires_at INTEGER NOT NULL,auth_version TEXT NOT NULL) STRICT",
        "CREATE TABLE server_devices(id TEXT PRIMARY KEY,name TEXT NOT NULL,token_hash TEXT NOT NULL,created_at INTEGER NOT NULL,revoked_at INTEGER,last_seen_at INTEGER) STRICT",
        "INSERT INTO server_admin VALUES(1,'admin','retained-password-hash','auth-version')",
        "INSERT INTO server_sessions VALUES('session-hash','{}',9999999999,'auth-version')",
        "PRAGMA application_id=1296323410", // MDSR
        "PRAGMA user_version=1",
        "CREATE TABLE server_assets(collision TEXT)", // Inject a mid-transition DDL failure.
    ] {
        raw.execute_unprepared(sql).await?;
    }
    assert!(ServerDatabase::open(&path).await.is_err());
    let row = raw
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT count(*) AS n FROM sqlite_schema WHERE name='server_sync_library'".to_owned(),
        ))
        .await?
        .ok_or("missing count")?;
    assert_eq!(row.try_get::<i64>("", "n")?, 0);
    raw.execute_unprepared("DROP TABLE server_assets").await?;
    raw.close().await?;
    let db = ServerDatabase::open(&path).await?;
    assert_eq!(
        db.admin().await?.ok_or("admin missing")?.password_hash,
        "retained-password-hash"
    );
    assert!(db.load_session("session-hash", 1).await?.is_some());
    let identity = db.sync_identity().await?;
    db.close().await?;
    let db = ServerDatabase::open(&path).await?;
    assert_eq!(db.sync_identity().await?.library_id, identity.library_id);
    assert_eq!(db.sync_identity().await?.epoch, identity.epoch);
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn failed_finalization_rolls_back_entities_receipts_and_cursor() -> Result {
    let dir = tempfile::tempdir()?;
    let db = ServerDatabase::open(dir.path().join("server.sqlite")).await?;
    let device = DeviceId::new();
    db.create_device(&device.to_string(), "test", &"0".repeat(64), 0)
        .await?;
    let tx = db.begin_sync(device).await?;
    let id = memedock_domain::identity::TagId::new();
    let entity = memedock_domain::tag::Tag::new(id, "tag".parse()?, TimestampMs::new(0));
    let operation = OperationEnvelope::new(
        tx.identity.library_id,
        tx.identity.epoch,
        device,
        OperationId::new(),
        TimestampMs::new(0),
        Operation::new(OperationKind::CreateTag {
            tag_id: id,
            name: "tag".parse()?,
        })?,
    );
    tx.save("tag", &id.to_string(), &entity, None, None).await?;
    let receipt = OperationReceipt {
        device_id: device,
        op_id: operation.op_id(),
        outcome: ReceiptOutcome::Accepted {
            event_seq: EventSeq::new(1)?,
            note: None,
        },
    };
    // An invalid event must fail before committing the preceding entity update.
    let wrong = memedock_domain::sync::SyncEvent::new(
        EventSeq::new(2)?,
        device,
        operation.op_id(),
        TimestampMs::new(0),
        memedock_domain::sync::EventKind::CollectionsReordered { items: vec![] },
    )?;
    assert!(
        tx.finish(&operation, &receipt, Some(&wrong), TimestampMs::new(0))
            .await
            .is_err()
    );
    let tx = db.begin_sync(device).await?;
    assert_eq!(tx.identity.head.get(), 0);
    assert!(
        tx.record::<memedock_domain::tag::Tag>("tag", &id.to_string())
            .await?
            .is_none()
    );
    assert!(tx.prior_receipt(&operation).await?.is_none());
    drop(tx);
    db.close().await?;
    Ok(())
}
