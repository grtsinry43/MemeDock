use crate::{CoreError, ErrorCode, ResourceLimits, Result};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Reserve the complete configured working allowance before entering a blocking
/// decoder. Waiting consumes neither a native worker nor pixel buffers.
pub(crate) struct ImageBudget {
    bytes: Arc<Semaphore>,
    request: u32,
}
impl ImageBudget {
    pub(crate) fn new(limits: &ResourceLimits) -> Result<Self> {
        let units = limits.image_budget_bytes / 1024;
        let request = limits.max_decode_bytes.div_ceil(1024);
        Ok(Self {
            bytes: Arc::new(Semaphore::new(usize::try_from(units).map_err(|_| {
                CoreError::new(ErrorCode::ResourceLimit, "image budget overflow")
            })?)),
            request: u32::try_from(request)
                .map_err(|_| CoreError::new(ErrorCode::ResourceLimit, "decode budget overflow"))?,
        })
    }
    pub(crate) async fn acquire(&self) -> Result<OwnedSemaphorePermit> {
        self.bytes
            .clone()
            .acquire_many_owned(self.request)
            .await
            .map_err(|_| CoreError::internal("image budget closed"))
    }
}
