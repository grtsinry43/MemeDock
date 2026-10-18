use memedock_server::{auth, config::ServerConfig, state::AppState};
use memedock_storage::server::ServerDatabase;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::Arc};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const PASSWORD: &str = "memedock-test-password";

struct Server {
    directory: tempfile::TempDir,
    db: ServerDatabase,
    origin: String,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let db = ServerDatabase::open(directory.path().join("server.sqlite")).await?;
        let hash = tokio::task::spawn_blocking(|| {
            auth::hash_password(PASSWORD).map_err(|e| e.0.message())
        })
        .await??;
        db.initialize_admin("admin", &hash, "initial").await?;
        let (origin, task) = spawn(db.clone(), directory.path().to_owned()).await?;
        Ok(Self {
            directory,
            db,
            origin,
            task,
        })
    }
    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.origin)
    }
    async fn restart(&mut self) -> TestResult {
        self.task.abort();
        let _ = (&mut self.task).await;
        let reopened = ServerDatabase::open(self.directory.path().join("server.sqlite")).await?;
        let previous = std::mem::replace(&mut self.db, reopened);
        previous.close().await?;
        let (origin, task) = spawn(self.db.clone(), self.directory.path().to_owned()).await?;
        self.origin = origin;
        self.task = task;
        Ok(())
    }
    async fn login(&self, client: &Client) -> Result<reqwest::Response, reqwest::Error> {
        client
            .post(self.url("/admin/login"))
            .header("Origin", &self.origin)
            .json(&json!({"username":"admin","password":PASSWORD}))
            .send()
            .await
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn spawn(
    db: ServerDatabase,
    data_dir: std::path::PathBuf,
) -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let listen = listener.local_addr()?;
    let origin = format!("http://{listen}");
    let router = memedock_server::router(AppState::new(
        db,
        ServerConfig {
            data_dir,
            listen,
            public_origin: origin.clone(),
        },
    ))?;
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("test server: {error}");
        }
    });
    Ok((origin, task))
}

#[tokio::test]
async fn real_http_device_lifecycle_and_authorization_boundaries() -> TestResult {
    let mut server = Server::start().await?;
    let jar = Arc::new(reqwest::cookie::Jar::default());
    let admin = Client::builder().cookie_provider(jar.clone()).build()?;
    let anonymous = Client::new();
    assert_eq!(
        admin.get(server.url("/admin/me")).send().await?.status(),
        StatusCode::UNAUTHORIZED
    );
    let denied = anonymous
        .post(server.url("/admin/login"))
        .json(&json!({"username":"admin","password":PASSWORD}))
        .send()
        .await?;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let denied = anonymous
        .post(server.url("/admin/login"))
        .header("Origin", "https://untrusted.invalid")
        .json(&json!({"username":"admin","password":PASSWORD}))
        .send()
        .await?;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let wrong = admin
        .post(server.url("/admin/login"))
        .header("Origin", &server.origin)
        .json(&json!({"username":"admin","password":"wrong"}))
        .send()
        .await?;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let response = server.login(&admin).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .ok_or("cookie missing")?
        .to_str()?;
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let request_id = response
        .headers()
        .get("x-request-id")
        .ok_or("request id missing")?
        .to_str()?
        .to_owned();
    let envelope: Value = response.json().await?;
    assert_eq!(envelope["meta"]["requestId"], request_id);
    assert_eq!(envelope["code"], 0);
    let created = admin
        .post(server.url("/admin/devices"))
        .header("Origin", &server.origin)
        .json(&json!({"name":"Phone"}))
        .send()
        .await?;
    assert_eq!(created.status(), StatusCode::OK);
    let created: Value = created.json().await?;
    let token = created["data"]["token"].as_str().ok_or("token missing")?;
    let id = created["data"]["device"]["id"]
        .as_str()
        .ok_or("id missing")?;
    let list = admin
        .get(server.url("/admin/devices"))
        .send()
        .await?
        .text()
        .await?;
    assert!(!list.contains(token) && !list.contains("tokenHash"));
    assert_eq!(
        anonymous
            .get(server.url("/admin/me"))
            .bearer_auth(token)
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        admin
            .get(server.url("/device/verify"))
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        anonymous
            .get(server.url("/device/verify"))
            .bearer_auth(token)
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    server.restart().await?;
    assert_eq!(
        admin.get(server.url("/admin/me")).send().await?.status(),
        StatusCode::OK
    );
    assert_eq!(
        anonymous
            .get(server.url("/device/verify"))
            .bearer_auth(token)
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        admin
            .delete(server.url(&format!("/admin/devices/{id}")))
            .header("Origin", &server.origin)
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        anonymous
            .get(server.url("/device/verify"))
            .bearer_auth(token)
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let malformed = admin
        .post(server.url("/admin/devices"))
        .header("Origin", &server.origin)
        .json(&json!({"name":""}))
        .send()
        .await?;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let missing = admin.get(server.url("/missing")).send().await?;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing.json::<Value>().await?["bizErr"], "NOT_FOUND");
    assert_eq!(
        admin
            .post(server.url("/admin/logout"))
            .header("Origin", &server.origin)
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        admin.get(server.url("/admin/me")).send().await?.status(),
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

#[tokio::test]
async fn changing_password_invalidates_all_sessions_but_preserves_device_credentials() -> TestResult
{
    let server = Server::start().await?;
    let first = Client::builder().cookie_store(true).build()?;
    let second = Client::builder().cookie_store(true).build()?;
    assert_eq!(server.login(&first).await?.status(), StatusCode::OK);
    assert_eq!(server.login(&second).await?.status(), StatusCode::OK);
    let response: Value = first
        .post(server.url("/admin/devices"))
        .header("Origin", &server.origin)
        .json(&json!({"name":"Laptop"}))
        .send()
        .await?
        .json()
        .await?;
    let token = response["data"]["token"].as_str().ok_or("token missing")?;
    let response = first
        .put(server.url("/admin/password"))
        .header("Origin", &server.origin)
        .json(&json!({"oldPassword":PASSWORD,"newPassword":"new-memedock-test-password"}))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    for client in [&first, &second] {
        assert_eq!(
            client.get(server.url("/admin/me")).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        server.login(&first).await?.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        first
            .post(server.url("/admin/login"))
            .header("Origin", &server.origin)
            .json(&json!({"username":"admin","password":"new-memedock-test-password"}))
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        Client::new()
            .get(server.url("/device/verify"))
            .bearer_auth(token)
            .send()
            .await?
            .status(),
        StatusCode::OK
    );
    Ok(())
}

#[tokio::test]
async fn production_origin_requires_https() -> TestResult {
    let config = ServerConfig {
        data_dir: "/unused".into(),
        listen: "0.0.0.0:8787".parse()?,
        public_origin: "http://example.com".into(),
    };
    assert!(config.validate().is_err());
    let config = ServerConfig {
        public_origin: "https://example.com".into(),
        ..config
    };
    assert!(config.validate()?);
    Ok(())
}

/// Only Playwright starts this explicit localhost test fixture; production has no seed endpoint.
#[tokio::test]
#[ignore = "long-running localhost browser fixture, started by Playwright"]
async fn browser_fixture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = ServerDatabase::open(dir.path().join("server.sqlite")).await?;
    let hash =
        tokio::task::spawn_blocking(|| auth::hash_password(PASSWORD).map_err(|e| e.0.message()))
            .await??;
    db.initialize_admin("admin", &hash, "browser-test").await?;
    let listen: SocketAddr = "127.0.0.1:18787".parse()?;
    let config = ServerConfig {
        data_dir: dir.path().into(),
        listen,
        public_origin: format!("http://{listen}"),
    };
    let router = memedock_server::router(AppState::new(db, config))?;
    axum::serve(tokio::net::TcpListener::bind(listen).await?, router).await?;
    Ok(())
}
