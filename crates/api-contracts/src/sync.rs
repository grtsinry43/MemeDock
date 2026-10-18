use memedock_domain::{
    sync::{OperationEnvelope, OperationReceipt},
    version::ProtocolVersion,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct SyncInfo {
    pub protocol_version: u32,
    pub library_id: String,
    pub sync_epoch: String,
    pub device_id: String,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub head: i64,
    pub max_push_bytes: u32,
    pub max_pull_events: u32,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub max_asset_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushRequest {
    pub operation: OperationEnvelope,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub protocol_version: ProtocolVersion,
    pub receipt: OperationReceipt,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullRequest {
    pub library_id: String,
    pub sync_epoch: String,
    pub after: i64,
    #[serde(default = "page_size")]
    pub limit: u32,
}
fn page_size() -> u32 {
    128
}
