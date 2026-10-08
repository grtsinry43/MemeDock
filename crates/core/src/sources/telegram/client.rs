use crate::{CoreError, ErrorCode, Result, tasks::TaskControl};
use serde::{Deserialize, de::DeserializeOwned};
use std::{sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub(super) struct Client {
    http: reqwest::Client,
    token: Arc<str>,
    base: reqwest::Url,
}
#[derive(Deserialize)]
struct Response<T> {
    ok: bool,
    result: Option<T>,
    error_code: Option<u16>,
}
#[derive(Deserialize)]
struct RemoteFile {
    file_path: Option<String>,
}

fn network(error: reqwest::Error) -> CoreError {
    // reqwest errors can embed a URL containing the bot token. Never retain them.
    CoreError::new(
        if error.is_timeout() {
            ErrorCode::Timeout
        } else {
            ErrorCode::Network
        },
        "telegram request failed",
    )
}
impl Client {
    pub(super) fn new(token: String) -> Result<Self> {
        Self::with_base(
            token,
            reqwest::Url::parse("https://api.telegram.org/")
                .map_err(|_| CoreError::internal("invalid API endpoint"))?,
        )
    }
    pub(super) fn with_base(token: String, base: reqwest::Url) -> Result<Self> {
        let Some((number, secret)) = token.split_once(':') else {
            return Err(CoreError::new(ErrorCode::InvalidInput, "invalid bot token"));
        };
        if number.is_empty()
            || !number.bytes().all(|b| b.is_ascii_digit())
            || secret.len() < 20
            || secret.len() > 128
            || !secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(CoreError::new(ErrorCode::InvalidInput, "invalid bot token"));
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
            .read_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(120))
            .user_agent("MemeDock/0.1")
            .build()
            .map_err(network)?;
        Ok(Self {
            http,
            token: token.into(),
            base,
        })
    }
    fn url(&self, path: &str) -> Result<reqwest::Url> {
        self.base
            .join(&format!("./{path}"))
            .map_err(|_| CoreError::new(ErrorCode::CorruptData, "invalid telegram file path"))
    }
    pub(super) async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        body: serde_json::Value,
        control: &TaskControl,
    ) -> Result<T> {
        control.check()?;
        let request = self
            .http
            .post(self.url(&format!("bot{}/{method}", self.token))?)
            .json(&body)
            .send();
        let mut response = tokio::select! { result = request => result.map_err(network)?, _ = control.cancelled() => return Err(CoreError::new(ErrorCode::Cancelled, "task cancelled")) };
        let mut bytes = Vec::new();
        while let Some(chunk) = tokio::select! { result = response.chunk() => result.map_err(network)?, _ = control.cancelled() => return Err(CoreError::new(ErrorCode::Cancelled, "task cancelled")) }
        {
            if bytes.len().saturating_add(chunk.len()) > 4 * 1024 * 1024 {
                return Err(CoreError::new(
                    ErrorCode::ResourceLimit,
                    "telegram response exceeds limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let reply: Response<T> = serde_json::from_slice(&bytes)
            .map_err(|_| CoreError::new(ErrorCode::CorruptData, "invalid telegram response"))?;
        if !reply.ok {
            return Err(CoreError::new(
                match reply.error_code.unwrap_or(response.status().as_u16()) {
                    401 | 403 => ErrorCode::Unauthorized,
                    429 => ErrorCode::RateLimited,
                    400 | 404 => ErrorCode::NotFound,
                    _ => ErrorCode::Network,
                },
                "telegram API rejected request",
            ));
        }
        if !response.status().is_success() {
            return Err(CoreError::new(
                ErrorCode::Network,
                "telegram HTTP request failed",
            ));
        }
        reply
            .result
            .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "telegram result missing"))
    }
    pub(super) async fn download(
        &self,
        file_id: &str,
        path: &std::path::Path,
        limit: u64,
        control: &TaskControl,
    ) -> Result<()> {
        let file: RemoteFile = self
            .call(
                "getFile",
                serde_json::json!({ "file_id": file_id }),
                control,
            )
            .await?;
        let file_path = file
            .file_path
            .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "telegram file path missing"))?;
        if file_path.is_empty()
            || !file_path.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            })
        {
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "invalid telegram file path",
            ));
        }
        let request = self
            .http
            .get(self.url(&format!("file/bot{}/{file_path}", self.token))?)
            .send();
        let mut response = tokio::select! { result = request => result.map_err(network)?, _ = control.cancelled() => return Err(CoreError::new(ErrorCode::Cancelled, "task cancelled")) };
        if !response.status().is_success() {
            return Err(CoreError::new(
                ErrorCode::Network,
                "telegram download failed",
            ));
        }
        if response.content_length().is_some_and(|n| n > limit) {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "telegram file exceeds limit",
            ));
        }
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .await?;
        let mut size = 0u64;
        while let Some(chunk) = tokio::select! { result = response.chunk() => result.map_err(network)?, _ = control.cancelled() => return Err(CoreError::new(ErrorCode::Cancelled, "task cancelled")) }
        {
            size = size.checked_add(chunk.len() as u64).ok_or_else(|| {
                CoreError::new(ErrorCode::ResourceLimit, "download size overflow")
            })?;
            if size > limit {
                return Err(CoreError::new(
                    ErrorCode::ResourceLimit,
                    "telegram file exceeds limit",
                ));
            }
            output.write_all(&chunk).await?;
        }
        control.check()?;
        output.sync_all().await?;
        Ok(())
    }
}
