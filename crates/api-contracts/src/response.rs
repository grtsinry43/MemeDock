use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ApiMeta {
    pub request_id: String,
    /// UTC RFC3339 timestamp.
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ApiEnvelope<T> {
    pub code: u32,
    pub biz_err: String,
    pub msg: String,
    pub data: T,
    pub meta: ApiMeta,
}
