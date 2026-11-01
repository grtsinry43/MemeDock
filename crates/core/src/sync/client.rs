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
    let category = if error.is_timeout() {
        "timeout"
    } else if error.is_dns() {
        "DNS resolution"
    } else if error.is_connect() {
        "TCP/TLS connection"
    } else if error.is_body() {
        "body transfer"
    } else {
        "HTTP transport"
    };
    let error = error.without_url();
    let mut detail = format!("{category}: {error}");
    let mut source = std::error::Error::source(&error);
    for _ in 0..4 {
        let Some(cause) = source else {
            break;
        };
        detail.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    CoreError::caused(
        if error.is_timeout() {
            ErrorCode::Timeout
        } else {
            ErrorCode::Network
        },
        "sync request failed",
        error,
    )
    .with_sync_diagnostic(memedock_domain::sync::diagnostics::SyncDiagnostic {
        detail,
        ..Default::default()
    })
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
            .header("x-memedock-sync-protocol", "2")
    }
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        control: &TaskControl,
    ) -> Result<reqwest::Response> {
        control.check()?;
        let request = request.build().map_err(network)?;
        let stage = format!("{} {}", request.method(), request.url().path());
        tokio::select! { response=self.http.execute(request)=>response.map_err(|e| {
            let mut error = network(e);
            let mut diagnostic = error.sync_diagnostic().cloned().unwrap_or_default();
            diagnostic.stage = stage;
            error = error.with_sync_diagnostic(diagnostic);
            error
        }), _=control.cancelled()=>Err(CoreError::new(ErrorCode::Cancelled,"sync cancelled")) }
    }
    async fn failure(&self, mut response: reqwest::Response, control: &TaskControl) -> CoreError {
        let status = response.status().as_u16();
        let stage = format!("HTTP {}", response.url().path());
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(retry_after_ms);
        let mut bytes = Vec::new();
        loop {
            let chunk = tokio::select! {
                chunk=response.chunk()=>chunk,
                _=control.cancelled()=>return CoreError::new(ErrorCode::Cancelled, "sync cancelled"),
            };
            match chunk {
                Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= 64 * 1024 => {
                    bytes.extend_from_slice(&chunk)
                }
                _ => break,
            }
        }
        let envelope =
            serde_json::from_slice::<ApiEnvelope<Option<serde_json::Value>>>(&bytes).ok();
        let code = match status {
            401 | 403 => ErrorCode::Unauthorized,
            404 => ErrorCode::NotFound,
            409 => ErrorCode::Conflict,
            413 => ErrorCode::ResourceLimit,
            429 => ErrorCode::RateLimited,
            426 => ErrorCode::UnsupportedSchema,
            500..=599 => ErrorCode::Network,
            _ => ErrorCode::InvalidInput,
        };
        let detail = envelope
            .as_ref()
            .map(|e| e.msg.replace(&self.token, "[redacted]"))
            .unwrap_or_else(|| format!("server returned HTTP {status}"));
        CoreError::new(code, "sync HTTP request failed").with_sync_diagnostic(
            memedock_domain::sync::diagnostics::SyncDiagnostic {
                stage,
                http_status: Some(status),
                detail,
                request_id: request_id
                    .or_else(|| envelope.as_ref().map(|e| e.meta.request_id.clone()))
                    .map(|value| value.replace(&self.token, "[redacted]")),
                business_code: envelope.map(|e| e.biz_err.replace(&self.token, "[redacted]")),
                next_retry_ms: retry.and_then(|ms| {
                    crate::writes::now()
                        .ok()
                        .map(|n| n.get().saturating_add(ms))
                }),
                ..Default::default()
            },
        )
    }
    async fn json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        control: &TaskControl,
    ) -> Result<T> {
        let mut response = self.send(request, control).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(self.failure(response, control).await);
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
    pub(super) async fn check_protocol(&self, control: &TaskControl) -> Result<()> {
        if self.info(control).await?.protocol_version
            != memedock_domain::version::ProtocolVersion::CURRENT.get()
        {
            return Err(CoreError::new(
                ErrorCode::UnsupportedSchema,
                "sync protocol mismatch; upgrade server and client to 0.2.1",
            ));
        }
        Ok(())
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
            _ => return Err(self.failure(response, control).await),
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
        if !response.status().is_success() {
            return Err(self.failure(response, control).await);
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

fn retry_after_ms(value: &str) -> Option<i64> {
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.min((i64::MAX / 1_000) as u64) as i64 * 1_000);
    }
    // IMF-fixdate, the HTTP-date format mandated for senders by RFC 9110.
    let fields: Vec<_> = value.split_ascii_whitespace().collect();
    if fields.len() != 6
        || fields[5] != "GMT"
        || !["Mon,", "Tue,", "Wed,", "Thu,", "Fri,", "Sat,", "Sun,"].contains(&fields[0])
    {
        return None;
    }
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == fields[2])? as i64
        + 1;
    let year: i64 = fields[3].parse().ok()?;
    let day: i64 = fields[1].parse().ok()?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][(month - 1) as usize];
    if day > max_day {
        return None;
    }
    let clock: Vec<_> = fields[4].split(':').collect();
    if clock.len() != 3 || !(1..=31).contains(&day) || !(1970..=9999).contains(&year) {
        return None;
    }
    let h: i64 = clock[0].parse().ok()?;
    let m: i64 = clock[1].parse().ok()?;
    let s: i64 = clock[2].parse().ok()?;
    if h > 23 || m > 59 || s > 59 || h < 0 || m < 0 || s < 0 {
        return None;
    }
    let y = year - i64::from(month <= 2);
    let era = y / 400;
    let yo = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let days = era * 146097 + yo * 365 + yo / 4 - yo / 100 + (153 * mp + 2) / 5 + day - 1 - 719468;
    let at = (days * 86400 + h * 3600 + m * 60 + s) * 1000;
    Some(at.saturating_sub(crate::writes::now().ok()?.get()).max(0))
}

#[cfg(test)]
mod tests {
    use super::retry_after_ms;

    #[test]
    fn retry_after_accepts_seconds_and_http_dates_with_bounded_invalid_input() {
        assert_eq!(retry_after_ms("120"), Some(120_000));
        assert_eq!(retry_after_ms("0"), Some(0));
        assert_eq!(retry_after_ms("999999999"), Some(999_999_999_000));
        assert_eq!(retry_after_ms("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert!(retry_after_ms("Sun, 01 Nov 2099 00:00:00 GMT").is_some_and(|ms| ms > 86_400_000));
        for invalid in [
            "-1",
            "later",
            "Mon, 31 Feb 2099 00:00:00 GMT",
            "Mon, 01 Jan 2099 24:00:00 GMT",
        ] {
            assert_eq!(retry_after_ms(invalid), None);
        }
    }
}
