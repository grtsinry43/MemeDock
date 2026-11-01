//! Retention policy for diagnostics persisted with the sync binding.
use super::Binding;
use memedock_domain::sync::diagnostics::SyncDiagnostic;

pub fn record(binding: &mut Binding, diagnostic: SyncDiagnostic) {
    let diagnostic = diagnostic.bounded();
    binding.diagnostic = Some(diagnostic.clone());
    binding.recent_errors.push(diagnostic);
    if binding.recent_errors.len() > 20 {
        binding
            .recent_errors
            .drain(..binding.recent_errors.len() - 20);
    }
}
