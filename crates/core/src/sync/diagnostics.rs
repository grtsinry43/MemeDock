use crate::{CoreError, ErrorCode, Result};
use memedock_domain::sync::diagnostics::SyncDiagnostic;
use memedock_storage::client_sync::Binding;

pub(super) fn diagnostic(
    error: &CoreError,
    stage: &str,
    hash: Option<String>,
    attempts: u32,
) -> Result<SyncDiagnostic> {
    let mut value = error.sync_diagnostic().cloned().unwrap_or_default();
    if value.stage.is_empty() {
        value.stage = stage.into();
    }
    value.asset_hash = hash.or(value.asset_hash);
    value.code = error.code().as_str().into();
    if value.detail.is_empty() {
        value.detail = error.message().into();
    }
    value.occurred_at_ms = crate::writes::now()?.get();
    value.attempts = value.attempts.max(attempts);
    Ok(value.bounded())
}
pub(super) fn delay(attempts: u32) -> i64 {
    let base = 1_000i64.saturating_mul(1i64 << attempts.min(8));
    // UUID randomness is independent across devices, without another RNG or
    // shared lock. Keep the final delay within a five-minute retry budget.
    let jitter = i64::from(uuid::Uuid::now_v7().as_bytes()[15]) * base / 512;
    (base + jitter).min(300_000)
}
pub(super) fn failed(
    binding: &mut Binding,
    error: &CoreError,
    stage: &str,
    hash: Option<String>,
    item: bool,
) -> Result<()> {
    binding.last_error = Some(error.code().as_str().into());
    binding.failures = binding.failures.saturating_add(1);
    let mut value = diagnostic(error, stage, hash, binding.failures)?;
    if !item
        && matches!(
            error.code(),
            ErrorCode::Network | ErrorCode::Timeout | ErrorCode::RateLimited | ErrorCode::Busy
        )
    {
        let fallback = value.occurred_at_ms.saturating_add(delay(binding.failures));
        binding.cooldown_until_ms = value.next_retry_ms.unwrap_or(fallback).max(fallback);
        value.next_retry_ms = Some(binding.cooldown_until_ms);
    }
    memedock_storage::client_sync::diagnostics::record(binding, value);
    Ok(())
}
