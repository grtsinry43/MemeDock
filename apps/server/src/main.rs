use memedock_server::{auth, config::ServerConfig, state::AppState};
use memedock_storage::server::ServerDatabase;
use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if command == "help" || command == "--help" {
        println!(
            "MemeDock server\n  init-admin --data-dir PATH --username NAME\n  reset-password --data-dir PATH\n  serve --data-dir PATH [--listen 127.0.0.1:8787] [--public-origin http://127.0.0.1:8787]\nPasswords are read interactively and never accepted as command-line arguments."
        );
        return Ok(());
    }
    if !matches!(command.as_str(), "serve" | "init-admin" | "reset-password") {
        return Err("unknown command; use --help".into());
    }
    let mut directory = None;
    let mut username = None;
    let mut listen: SocketAddr = "127.0.0.1:8787".parse()?;
    let mut origin = None;
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing option value")?;
        match flag.as_str() {
            "--data-dir" => directory = Some(PathBuf::from(value)),
            "--username" if command == "init-admin" => username = Some(value),
            "--listen" if command == "serve" => listen = value.parse()?,
            "--public-origin" if command == "serve" => origin = Some(value),
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let directory = directory.ok_or("--data-dir is required")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&directory)?;
    // One serving process per data directory makes startup staging recovery safe.
    // Administrative password commands remain usable while the service runs.
    let _data_lock = if command == "serve" {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("server.lock"))?;
        lock.try_lock()
            .map_err(|_| "another server is already using this data directory")?;
        Some(lock)
    } else {
        None
    };
    let db = ServerDatabase::open(directory.join("server.sqlite")).await?;
    if command != "serve" {
        let previous = db.admin().await?;
        if command == "init-admin" && previous.is_some() {
            return Err("administrator already initialized".into());
        }
        let username = if command == "init-admin" {
            let value = username.ok_or("--username is required")?;
            if value.trim() != value
                || value.is_empty()
                || value.chars().count() > 80
                || value.chars().any(char::is_control)
            {
                return Err("invalid username".into());
            }
            value
        } else {
            previous
                .as_ref()
                .ok_or("administrator is not initialized")?
                .username
                .clone()
        };
        let hash = tokio::task::spawn_blocking(|| -> Result<String, String> {
            let password = rpassword::prompt_password("Password (12–1024 bytes): ")
                .map_err(|e| e.to_string())?;
            let confirmation =
                rpassword::prompt_password("Confirm password: ").map_err(|e| e.to_string())?;
            if password != confirmation {
                return Err("passwords do not match".into());
            }
            auth::hash_password(&password).map_err(|e| e.0.message().to_owned())
        })
        .await??;
        let version = uuid::Uuid::now_v7().to_string();
        if let Some(previous) = previous {
            db.replace_password(&previous.auth_version, &hash, &version)
                .await?;
        } else {
            db.initialize_admin(&username, &hash, &version).await?;
        }
        println!("Administrator configured; previous browser sessions are invalid.");
        db.close().await?;
        return Ok(());
    }
    if db.admin().await?.is_none() {
        return Err("run init-admin before serving".into());
    }
    if !cfg!(feature = "web") {
        return Err("serve requires embedded Web assets; build with node tools/server/build.mjs or cargo build -p memedock-server --features web after building the frontend".into());
    }
    let originals = directory.join("originals");
    tokio::task::spawn_blocking(move || {
        memedock_storage::files::FsBlobStore::open(originals)?.discard_abandoned_staging()
    })
    .await??;
    let config = ServerConfig {
        data_dir: directory,
        listen,
        public_origin: origin.unwrap_or_else(|| format!("http://{listen}")),
    };
    let state = AppState::new(db.clone(), config);
    if let Err(error) = memedock_core::server_sync::cleanup_purged(
        &db,
        &state.config.data_dir.join("originals"),
        &state.config.data_dir.join("cache"),
    )
    .await
    {
        eprintln!("permanent-delete cleanup retry failed: {error}");
    }
    let router = memedock_server::router(state)?;
    let listener = tokio::net::TcpListener::bind(listen).await?;
    println!("MemeDock server listening on {listen}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    db.close().await?;
    Ok(())
}
