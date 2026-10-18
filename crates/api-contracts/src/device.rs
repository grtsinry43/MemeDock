use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct CreateDeviceRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct DeviceList {
    pub items: Vec<DeviceInfo>,
}

/// Returned only when creating/replacing credentials. Deliberately not Debug.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct DeviceCredential {
    pub device: DeviceInfo,
    pub token: String,
}
