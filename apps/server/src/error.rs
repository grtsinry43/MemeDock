use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use memedock_api_contracts::{ApiEnvelope, ApiErrorKind, ApiMeta};
use memedock_storage::StorageError;
use time::OffsetDateTime;

pub struct ApiError(pub ApiErrorKind);
impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::Conflict(_) => Self(ApiErrorKind::Conflict),
            StorageError::InvalidInput(_) => Self(ApiErrorKind::ParamsError),
            _ => {
                eprintln!("server storage failure: {error}");
                Self(ApiErrorKind::ServerError)
            }
        }
    }
}
impl From<tower_sessions::session::Error> for ApiError {
    fn from(error: tower_sessions::session::Error) -> Self {
        eprintln!("session failure: {error}");
        Self(ApiErrorKind::ServerError)
    }
}

#[derive(Clone)]
pub struct RequestContext {
    pub id: String,
    pub timestamp: String,
}
impl Default for RequestContext {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            timestamp: format_time(OffsetDateTime::now_utc()),
        }
    }
}
impl RequestContext {
    pub fn respond<T: serde::Serialize>(&self, result: Result<T, ApiError>) -> Response {
        let meta = ApiMeta {
            request_id: self.id.clone(),
            timestamp: self.timestamp.clone(),
        };
        match result {
            Ok(data) => Json(ApiEnvelope {
                code: 0,
                biz_err: "OK".into(),
                msg: "success".into(),
                data,
                meta,
            })
            .into_response(),
            Err(ApiError(kind)) => {
                eprintln!("request_id={} error={}", self.id, kind.name());
                (
                    StatusCode::from_u16(kind.status())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(ApiEnvelope {
                        code: kind.code(),
                        biz_err: kind.name().into(),
                        msg: kind.message().into(),
                        data: Option::<()>::None,
                        meta,
                    }),
                )
                    .into_response()
            }
        }
    }
}
pub fn timestamp(seconds: i64) -> Result<String, ApiError> {
    OffsetDateTime::from_unix_timestamp(seconds)
        .map(format_time)
        .map_err(|_| ApiError(ApiErrorKind::ServerError))
}
fn format_time(value: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        value.year(),
        value.month() as u8,
        value.day(),
        value.hour(),
        value.minute(),
        value.second()
    )
}
