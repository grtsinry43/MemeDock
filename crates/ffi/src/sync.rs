use crate::{LibraryHandle, Result, SyncStatusTask, SyncSubscriptionHandle};
use std::sync::Arc;

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncStatus {
    pub origin: Option<String>,
    pub device_id: Option<String>,
    pub connected: bool,
    pub paused: bool,
    pub pending: u64,
    pub blocked: u64,
    pub cursor: i64,
    pub last_success_ms: Option<i64>,
    pub error: Option<String>,
    pub retry_after_ms: u64,
}
impl From<memedock_core::sync::SyncStatus> for SyncStatus {
    fn from(v: memedock_core::sync::SyncStatus) -> Self {
        Self {
            origin: v.origin,
            device_id: v.device_id,
            connected: v.connected,
            paused: v.paused,
            pending: v.pending,
            blocked: v.blocked,
            cursor: v.cursor,
            last_success_ms: v.last_success_ms,
            error: v.error,
            retry_after_ms: v.retry_after_ms,
        }
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn sync_status(&self) -> Result<Arc<SyncStatusTask>> {
        Ok(SyncStatusTask::new(self.inner.sync_status()?))
    }
    pub fn connect_sync(&self, origin: String, token: String) -> Result<Arc<SyncStatusTask>> {
        Ok(SyncStatusTask::new(self.inner.connect_sync(origin, token)?))
    }
    pub fn synchronize(&self, token: String) -> Result<Arc<SyncStatusTask>> {
        Ok(SyncStatusTask::new(self.inner.synchronize(token)?))
    }
    pub fn set_sync_paused(&self, paused: bool) -> Result<Arc<SyncStatusTask>> {
        Ok(SyncStatusTask::new(self.inner.set_sync_paused(paused)?))
    }
    pub fn subscribe_sync(&self) -> Result<Arc<SyncSubscriptionHandle>> {
        Ok(SyncSubscriptionHandle::new(self.inner.subscribe_sync()?))
    }
}
