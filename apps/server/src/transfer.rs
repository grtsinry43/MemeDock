//! Bounded FIFO admission, with at most one active transfer per device.
//! Permits belong to the response body, so disconnects release every budget.
use crate::error::ApiError;
use memedock_api_contracts::ApiErrorKind;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MAX_ACTIVE_TRANSFERS: usize = 8;
const MAX_QUEUED_TRANSFERS: usize = 512;

pub struct TransferBudget {
    requests: Arc<Semaphore>,
    active: Arc<Semaphore>,
    devices: Mutex<HashMap<String, Weak<Semaphore>>>,
}
pub struct TransferPermit {
    _request: OwnedSemaphorePermit,
    _device: OwnedSemaphorePermit,
    _active: OwnedSemaphorePermit,
}
impl Default for TransferBudget {
    fn default() -> Self {
        Self {
            requests: Arc::new(Semaphore::new(MAX_ACTIVE_TRANSFERS + MAX_QUEUED_TRANSFERS)),
            active: Arc::new(Semaphore::new(MAX_ACTIVE_TRANSFERS)),
            devices: Mutex::new(HashMap::new()),
        }
    }
}
impl TransferBudget {
    pub async fn acquire(&self, device: String) -> Result<TransferPermit, ApiError> {
        let request = self
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError(ApiErrorKind::TransferQueueFull))?;
        let gate = {
            let mut devices = self
                .devices
                .lock()
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
            devices.retain(|_, value| value.strong_count() > 0);
            if let Some(gate) = devices.get(&device).and_then(Weak::upgrade) {
                gate
            } else {
                let gate = Arc::new(Semaphore::new(1));
                devices.insert(device, Arc::downgrade(&gate));
                gate
            }
        };
        tokio::time::timeout(Duration::from_secs(25), async {
            let device = gate
                .acquire_owned()
                .await
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
            let active = self
                .active
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
            Ok(TransferPermit {
                _request: request,
                _device: device,
                _active: active,
            })
        })
        .await
        .map_err(|_| ApiError(ApiErrorKind::TransferBusy))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn eight_devices_can_transfer_while_same_device_and_overflow_wait() -> Result {
        let budget = Arc::new(TransferBudget::default());
        let first = budget.acquire("a".into()).await.map_err(|_| "first")?;
        let b = budget.clone();
        let same_device =
            tokio::spawn(async move { b.acquire("a".into()).await.map_err(|_| "same device") });
        tokio::task::yield_now().await;
        assert!(!same_device.is_finished());
        let mut others = Vec::new();
        for i in 0..7 {
            others.push(
                budget
                    .acquire(format!("other-{i}"))
                    .await
                    .map_err(|_| "other device")?,
            );
        }
        assert_eq!(budget.active.available_permits(), 0);
        let b = budget.clone();
        let waiting =
            tokio::spawn(async move { b.acquire("c".into()).await.map_err(|_| "waiting") });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        drop(others.pop().ok_or("missing active transfer")?);
        let admitted = tokio::time::timeout(Duration::from_secs(1), waiting).await???;
        drop(admitted);
        drop(first);
        drop(tokio::time::timeout(Duration::from_secs(1), same_device).await???);
        drop(others);
        assert_eq!(budget.active.available_permits(), 8);
        assert_eq!(budget.requests.available_permits(), 520);
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_and_full_queue_do_not_leak_admission() -> Result {
        let budget = Arc::new(TransferBudget::default());
        let mut active = Vec::new();
        for i in 0..8 {
            active.push(
                budget
                    .acquire(format!("active-{i}"))
                    .await
                    .map_err(|_| "active")?,
            );
        }
        let mut queued = Vec::new();
        for i in 0..512 {
            let b = budget.clone();
            queued.push(tokio::spawn(async move {
                b.acquire(format!("queued-{i}")).await.map_err(|_| "queue")
            }));
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while budget.requests.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(budget.requests.available_permits(), 0);
        assert!(matches!(
            budget.acquire("overflow".into()).await,
            Err(ApiError(ApiErrorKind::TransferQueueFull))
        ));
        for task in queued {
            task.abort();
            let _ = task.await;
        }
        drop(active);
        assert_eq!(budget.active.available_permits(), 8);
        assert_eq!(budget.requests.available_permits(), 520);
        drop(
            budget
                .acquire("new-device".into())
                .await
                .map_err(|_| "admission leaked")?,
        );
        Ok(())
    }
}
