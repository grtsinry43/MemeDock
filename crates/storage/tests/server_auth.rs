use memedock_storage::{LibraryDatabase, StorageError, server::ServerDatabase};

#[tokio::test]
async fn initialization_is_singleton_and_sessions_cannot_resurrect_after_reset()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("server.sqlite");
    let db = ServerDatabase::open(&path).await?;
    let (a, b) = tokio::join!(
        db.initialize_admin("admin", "hash", "version1"),
        db.initialize_admin("other", "hash", "other")
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let original = db.admin().await?.ok_or("account missing")?;
    assert!(
        db.create_session("session", "{}", 100, &original.auth_version)
            .await?
    );
    assert!(
        !db.create_session("session", "{}", 100, &original.auth_version)
            .await?
    );
    assert!(db.load_session("session", 99).await?.is_some());
    assert!(db.load_session("session", 100).await?.is_none());
    db.replace_password(&original.auth_version, "newhash", "newversion")
        .await?;
    db.save_session("session", "{}", 200).await?;
    assert!(db.load_session("session", 0).await?.is_none());
    assert!(
        !db.create_session("stale-login", "{}", 200, &original.auth_version)
            .await?
    );
    assert!(
        db.replace_password(&original.auth_version, "oldhash", "oldversion")
            .await
            .is_err()
    );
    db.close().await?;
    let db = ServerDatabase::open(&path).await?;
    assert_eq!(
        db.admin().await?.ok_or("account missing")?.password_hash,
        "newhash"
    );
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn server_refuses_client_database_without_altering_it()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("library.sqlite");
    let client = LibraryDatabase::open(&path).await?;
    let identity = client.identity();
    client.close().await?;
    assert!(matches!(
        ServerDatabase::open(&path).await,
        Err(StorageError::UnsupportedSchema)
    ));
    let client = LibraryDatabase::open(&path).await?;
    assert_eq!(client.identity(), identity);
    client.close().await?;
    Ok(())
}
