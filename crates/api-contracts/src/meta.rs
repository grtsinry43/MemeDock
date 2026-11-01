use serde::{Deserialize, Serialize};

/// Public build identity of the running server, shown by the web console.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct VersionInfo {
    pub version: String,
    pub protocol_version: u32,
    pub debug: bool,
}
