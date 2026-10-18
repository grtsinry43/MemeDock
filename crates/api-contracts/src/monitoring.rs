use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum ActivityResult {
    Accepted,
    NoOp,
    Rejected,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ActivityQuery {
    pub device_id: Option<String>,
    pub result: Option<ActivityResult>,
    pub page: u32,
    pub limit: u32,
}
impl Default for ActivityQuery {
    fn default() -> Self {
        Self {
            device_id: None,
            result: None,
            page: 1,
            limit: 25,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct DeviceSyncProgress {
    pub id: String,
    pub name: String,
    pub revoked: bool,
    pub last_seen_at: Option<String>,
    #[cfg_attr(feature = "typescript", ts(type = "number | null"))]
    pub applied_cursor: Option<i64>,
    #[cfg_attr(feature = "typescript", ts(type = "number | null"))]
    pub observed_at: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct SyncOverview {
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub head: i64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub stickers: u64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub deleted: u64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub collections: u64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub tags: u64,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub original_bytes: u64,
    pub devices: Vec<DeviceSyncProgress>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ActivityEntry {
    pub op_id: String,
    pub device_id: String,
    pub device_name: String,
    pub kind: String,
    pub target: Option<String>,
    pub result: ActivityResult,
    pub error_code: Option<String>,
    #[cfg_attr(feature = "typescript", ts(type = "number | null"))]
    pub received_at: Option<i64>,
    #[cfg_attr(feature = "typescript", ts(type = "number | null"))]
    pub event_seq: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ActivityPage {
    pub items: Vec<ActivityEntry>,
    #[cfg_attr(feature = "typescript", ts(type = "number"))]
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}
