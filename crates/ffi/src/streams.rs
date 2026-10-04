use crate::{BridgeError, ErrorCode, Notification, Result, TaskSnapshot};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

struct Slot<T> {
    value: Option<T>,
    closed: bool,
}
/// One reader at a time. A lease returns its receiver even when its future is
/// dropped, preserving the cursor without holding a mutex across await.
struct Stream<T> {
    slot: Mutex<Slot<T>>,
    closed: watch::Sender<bool>,
}
impl<T> Stream<T> {
    fn new(value: T) -> Self {
        let (closed, _) = watch::channel(false);
        Self {
            slot: Mutex::new(Slot {
                value: Some(value),
                closed: false,
            }),
            closed,
        }
    }
    fn lease(&self) -> Result<Option<Lease<'_, T>>> {
        let mut slot = self
            .slot
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "stream slot poisoned"))?;
        if slot.closed {
            return Ok(None);
        }
        let value = slot.value.take().ok_or_else(|| {
            BridgeError::new(ErrorCode::Busy, "stream already has a waiting reader")
        })?;
        Ok(Some(Lease {
            stream: self,
            value: Some(value),
        }))
    }
    fn close(&self) -> Result<()> {
        let mut slot = self
            .slot
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "stream slot poisoned"))?;
        slot.closed = true;
        slot.value.take();
        self.closed.send_replace(true);
        Ok(())
    }
}
struct Lease<'a, T> {
    stream: &'a Stream<T>,
    value: Option<T>,
}
impl<T> Lease<'_, T> {
    fn get(&mut self) -> Result<&mut T> {
        self.value
            .as_mut()
            .ok_or_else(|| BridgeError::new(ErrorCode::Internal, "empty stream lease"))
    }
}
impl<T> Drop for Lease<'_, T> {
    fn drop(&mut self) {
        // Only moving owned receivers happens under this lock. Preserve cleanup
        // after poisoning without invoking user code or doing IO under the lock.
        let mut slot = self.stream.slot.lock().unwrap_or_else(|p| p.into_inner());
        if !slot.closed {
            slot.value = self.value.take();
        }
    }
}
#[derive(uniffi::Object)]
pub struct SubscriptionHandle {
    stream: Stream<memedock_core::events::Subscription>,
}
impl SubscriptionHandle {
    pub(crate) fn new(value: memedock_core::events::Subscription) -> Arc<Self> {
        Arc::new(Self {
            stream: Stream::new(value),
        })
    }
}
#[uniffi::export]
impl SubscriptionHandle {
    pub fn unsubscribe(&self) -> Result<()> {
        self.stream.close()
    }
    pub async fn next(&self) -> Result<Notification> {
        let Some(mut lease) = self.stream.lease()? else {
            return Ok(Notification::Closed);
        };
        let mut closed = self.stream.closed.subscribe();
        if *closed.borrow() {
            return Ok(Notification::Closed);
        }
        let value = tokio::select! {
            result = lease.get()?.next() => result.map(Into::into).map_err(BridgeError::from),
            _ = closed.changed() => Ok(Notification::Closed),
        }?;
        if value == Notification::Closed {
            self.stream.close()?;
        }
        Ok(value)
    }
}
#[derive(uniffi::Object)]
pub struct TaskProgressHandle {
    stream: Stream<memedock_core::tasks::TaskProgress>,
}
impl TaskProgressHandle {
    pub(crate) fn new(value: memedock_core::tasks::TaskProgress) -> Arc<Self> {
        Arc::new(Self {
            stream: Stream::new(value),
        })
    }
}
#[uniffi::export]
impl TaskProgressHandle {
    pub fn unsubscribe(&self) -> Result<()> {
        self.stream.close()
    }
    pub async fn next(&self) -> Result<Option<TaskSnapshot>> {
        let Some(mut lease) = self.stream.lease()? else {
            return Ok(None);
        };
        let mut closed = self.stream.closed.subscribe();
        if *closed.borrow() {
            return Ok(None);
        }
        let value = tokio::select! {
            result = lease.get()?.next() => result.map(Into::into),
            _ = closed.changed() => None,
        };
        if value.is_none() {
            self.stream.close()?;
        }
        Ok(value)
    }
}
