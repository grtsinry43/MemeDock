//! Bounded, credential-free sync diagnostics shared by persistence and clients.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncDiagnostic {
    pub stage: String,
    pub asset_hash: Option<String>,
    pub code: String,
    pub detail: String,
    pub http_status: Option<u16>,
    pub business_code: Option<String>,
    pub request_id: Option<String>,
    pub occurred_at_ms: i64,
    pub attempts: u32,
    pub next_retry_ms: Option<i64>,
}
impl SyncDiagnostic {
    pub fn description(&self) -> String {
        format!(
            "{} | {}{} | {}{}{}{} | attempts={} | at={}{}",
            self.stage,
            self.code,
            self.http_status
                .map(|s| format!(" HTTP {s}"))
                .unwrap_or_default(),
            self.detail,
            self.business_code
                .as_ref()
                .map(|s| format!(" | business={s}"))
                .unwrap_or_default(),
            self.request_id
                .as_ref()
                .map(|s| format!(" | request={s}"))
                .unwrap_or_default(),
            self.asset_hash
                .as_ref()
                .map(|s| format!(" | asset={s}"))
                .unwrap_or_default(),
            self.attempts,
            self.occurred_at_ms,
            self.next_retry_ms
                .map(|s| format!(" | retry_at={s}"))
                .unwrap_or_default()
        )
    }
    pub fn bounded(mut self) -> Self {
        fn limit(s: &mut String, n: usize) {
            if let Some((at, _)) = s.char_indices().nth(n) {
                s.truncate(at);
            }
        }
        limit(&mut self.stage, 64);
        limit(&mut self.code, 64);
        limit(&mut self.detail, 512);
        for value in [
            &mut self.asset_hash,
            &mut self.business_code,
            &mut self.request_id,
        ]
        .into_iter()
        .flatten()
        {
            limit(value, 128);
        }
        self
    }
}
