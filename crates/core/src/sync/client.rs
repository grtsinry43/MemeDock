use crate::{CoreError, ErrorCode, Result, tasks::TaskControl};
use memedock_api_contracts::{ApiEnvelope, PushRequest, PushResponse, SyncInfo};
use memedock_domain::{asset::Asset, sync::PullPage};
use memedock_storage::client_sync::Binding;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

pub(super) struct Client {
    http: reqwest::Client,
    origin: String,
    token: String,
}
pub(super) fn network(error: reqwest::Error) -> CoreError {
    CoreError::new(
        if error.is_timeout() {
            ErrorCode::Timeout
        } else {
            ErrorCode::Network
        },
        "sync request failed",
    )
}
impl Client {
    pub(super) fn new(origin: &str, token: String) -> Result<Self> {
        let url = reqwest::Url::parse(origin)
            .map_err(|_| CoreError::new(ErrorCode::InvalidInput, "invalid sync server address"))?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if url.cannot_be_a_base()
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "use an HTTPS server origin; HTTP is allowed only for loopback",
            ));
        }
        if token.len() != 105 || token.contains(['\r', '\n', '\0']) || !token.starts_with("mdk_") {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "invalid device token",
            ));
        }
        let certs = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(|der| reqwest::Certificate::from_der(der.as_ref()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| CoreError::internal("invalid TLS root certificate"))?;
        let http = reqwest::Client::builder()
            .tls_certs_only(certs)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(180))
            .user_agent(concat!("MemeDock/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(network)?;
        Ok(Self {
            http,
            origin: url.origin().ascii_serialization(),
            token,
        })
    }
    pub(super) fn origin(&self) -> &str {
        &self.origin
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}/api/v1{path}", self.origin))
            .bearer_auth(&self.token)
    }
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        control: &TaskControl,
    ) -> Result<reqwest::Response> {
        control.check()?;
        tokio::select! { response=request.send()=>response.map_err(network), _=control.cancelled()=>Err(CoreError::new(ErrorCode::Cancelled,"sync cancelled")) }
    }
    async fn json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        control: &TaskControl,
    ) -> Result<T> {
        let mut response = self.send(request, control).await?;
        let status = response.status();
        if matches!(status.as_u16(), 401 | 403) {
            return Err(CoreError::new(
                ErrorCode::Unauthorized,
                "device authorization is invalid or revoked",
            ));
        }
        if status.as_u16() == 429 {
            return Err(CoreError::new(
                ErrorCode::RateLimited,
                "sync server is busy",
            ));
        }
        if status.as_u16() == 413 {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "sync request exceeds server limit",
            ));
        }
        // Proxies commonly return HTML or an empty body for transient failures.
        // Keep these retryable without depending on the application's envelope.
        if status.is_server_error() {
            return Err(CoreError::new(
                ErrorCode::Network,
                "sync server unavailable",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = tokio::select! {chunk=response.chunk()=>chunk.map_err(network)?, _=control.cancelled()=>return Err(CoreError::new(ErrorCode::Cancelled,"sync cancelled"))}
        {
            if bytes.len().saturating_add(chunk.len()) > 20 * 1024 * 1024 {
                return Err(CoreError::new(
                    ErrorCode::ResourceLimit,
                    "sync response exceeds budget",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let envelope: ApiEnvelope<Option<T>> = serde_json::from_slice(&bytes)
            .map_err(|_| CoreError::new(ErrorCode::CorruptData, "incompatible sync response"))?;
        if !status.is_success() || envelope.code != 0 {
            return Err(CoreError::new(
                if status.as_u16() == 409 {
                    ErrorCode::Conflict
                } else if status.is_server_error() {
                    ErrorCode::Network
                } else {
                    ErrorCode::InvalidInput
                },
                "sync server rejected the request",
            ));
        }
        envelope
            .data
            .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "missing sync response"))
    }
    pub(super) async fn info(&self, control: &TaskControl) -> Result<SyncInfo> {
        self.json(
            self.request(reqwest::Method::GET, "/device/sync/info"),
            control,
        )
        .await
    }
    pub(super) async fn pull(&self, binding: &Binding, control: &TaskControl) -> Result<PullPage> {
        let page: PullPage = self
            .json(
                self.request(
                    reqwest::Method::GET,
                    &format!(
                        "/device/sync/pull?libraryId={}&syncEpoch={}&after={}&limit=128",
                        binding.library_id,
                        binding.epoch,
                        binding.cursor.get()
                    ),
                ),
                control,
            )
            .await?;
        page.ensure_identity(binding.library_id, binding.epoch)?;
        page.validate_after(binding.cursor)?;
        Ok(page)
    }
    pub(super) async fn push(
        &self,
        operation: memedock_domain::sync::OperationEnvelope,
        control: &TaskControl,
    ) -> Result<PushResponse> {
        let request = PushRequest { operation };
        if serde_json::to_vec(&request)
            .map_err(|_| CoreError::internal("sync serialization failed"))?
            .len()
            > 64 * 1024
        {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "sync operation exceeds server limit",
            ));
        }
        self.json(
            self.request(reqwest::Method::POST, "/device/sync/push")
                .json(&request),
            control,
        )
        .await
    }
    pub(super) async fn upload(
        &self,
        asset: &Asset,
        blobs: &memedock_storage::files::FsBlobStore,
        control: &TaskControl,
    ) -> Result<()> {
        let path = format!("/device/assets/{}", asset.hash());
        let response = self
            .send(self.request(reqwest::Method::HEAD, &path), control)
            .await?;
        match response.status().as_u16() {
            200 => return Ok(()),
            404 => {}
            401 | 403 => {
                return Err(CoreError::new(
                    ErrorCode::Unauthorized,
                    "device authorization is invalid or revoked",
                ));
            }
            429 => {
                return Err(CoreError::new(
                    ErrorCode::RateLimited,
                    "sync server is busy",
                ));
            }
            _ => return Err(CoreError::new(ErrorCode::Network, "asset check failed")),
        }
        let blobs = blobs.clone();
        let hash = asset.hash();
        let file = tokio::task::spawn_blocking(move || blobs.open_original(hash)).await??;
        let stream = ReaderStream::new(tokio::fs::File::from_std(file));
        let actual: Asset = self
            .json(
                self.request(reqwest::Method::PUT, &path)
                    .header(reqwest::header::CONTENT_LENGTH, asset.byte_size().get())
                    .body(reqwest::Body::wrap_stream(stream)),
                control,
            )
            .await?;
        if actual.hash() != asset.hash()
            || actual.byte_size() != asset.byte_size()
            || actual.width() != asset.width()
            || actual.height() != asset.height()
            || actual.format() != asset.format()
            || actual.animated() != asset.animated()
        {
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "uploaded asset metadata mismatch",
            ));
        }
        Ok(())
    }
    pub(super) async fn download(
        &self,
        asset: &Asset,
        pending: &memedock_storage::files::PendingStaging,
        control: &TaskControl,
    ) -> Result<()> {
        let mut response = self
            .send(
                self.request(
                    reqwest::Method::GET,
                    &format!("/device/assets/{}", asset.hash()),
                ),
                control,
            )
            .await?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(CoreError::new(
                ErrorCode::Unauthorized,
                "device authorization is invalid or revoked",
            ));
        }
        if !response.status().is_success() {
            return Err(CoreError::new(
                ErrorCode::Network,
                "original download failed",
            ));
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(pending.path())
            .await?;
        let mut bytes = 0u64;
        while let Some(chunk) = tokio::select! {chunk=response.chunk()=>chunk.map_err(network)?,_=control.cancelled()=>return Err(CoreError::new(ErrorCode::Cancelled,"sync cancelled"))}
        {
            bytes = bytes.checked_add(chunk.len() as u64).ok_or_else(|| {
                CoreError::new(ErrorCode::ResourceLimit, "original length overflow")
            })?;
            if bytes > asset.byte_size().get() as u64 {
                return Err(CoreError::new(
                    ErrorCode::CorruptData,
                    "original exceeds declared size",
                ));
            }
            file.write_all(&chunk).await?;
        }
        if bytes != asset.byte_size().get() as u64 {
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "original length mismatch",
            ));
        }
        file.sync_all().await?;
        Ok(())
    }
}
