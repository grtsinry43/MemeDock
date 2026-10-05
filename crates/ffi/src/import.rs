use crate::{BridgeError, ErrorCode, Result};
use std::sync::{Arc, Mutex};

/// A single-use staging ownership token. Close the platform writer before
/// submitting or discarding. Dropping leaves a file for next-session recovery.
#[derive(uniffi::Object)]
pub struct ImportInputHandle {
    inner: Mutex<Option<memedock_core::ImportInput>>,
    path: String,
}
impl ImportInputHandle {
    pub(crate) fn new(input: memedock_core::ImportInput) -> Result<Arc<Self>> {
        let path = input
            .path()
            .to_str()
            .ok_or_else(|| BridgeError::new(ErrorCode::InvalidInput, "staging path is not UTF-8"))?
            .to_owned();
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(input)),
            path,
        }))
    }
    pub(crate) fn take(&self) -> Result<memedock_core::ImportInput> {
        self.inner
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "input slot poisoned"))?
            .take()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "input already consumed"))
    }
}
#[uniffi::export]
impl ImportInputHandle {
    pub fn path(&self) -> String {
        self.path.clone()
    }
}
