use crate::{CoreError, ErrorCode, Result};
use memedock_domain::identity::StickerId;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    UsageChanged(StickerId),
    StickerChanged(StickerId),
    ThumbnailChanged(StickerId),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeEvent {
    pub sequence: u64,
    pub kind: ChangeKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notification {
    Changed(ChangeEvent),
    ReloadRequired { through_sequence: u64 },
    Closed,
}

struct EventState {
    sequence: u64,
    closed: bool,
    events: VecDeque<ChangeEvent>,
}
pub(crate) struct EventHub {
    state: Mutex<EventState>,
    signal: watch::Sender<u64>,
    subscribers: AtomicUsize,
    capacity: usize,
}
impl EventHub {
    pub(crate) fn new(capacity: usize) -> Arc<Self> {
        let (signal, _) = watch::channel(0);
        Arc::new(Self {
            state: Mutex::new(EventState {
                sequence: 0,
                closed: false,
                events: VecDeque::new(),
            }),
            signal,
            subscribers: AtomicUsize::new(0),
            capacity,
        })
    }
    pub(crate) fn subscribe(self: &Arc<Self>) -> Result<Subscription> {
        let state = self
            .state
            .lock()
            .map_err(|_| CoreError::internal("event state poisoned"))?;
        if state.closed {
            return Err(CoreError::new(
                ErrorCode::Closed,
                "library notifications closed",
            ));
        }
        // Both retained events and active subscriptions are bounded.
        if self.subscribers.load(Ordering::Relaxed) >= self.capacity {
            return Err(CoreError::new(
                ErrorCode::Busy,
                "subscription capacity reached",
            ));
        }
        self.subscribers.fetch_add(1, Ordering::Relaxed);
        Ok(Subscription {
            hub: self.clone(),
            receiver: self.signal.subscribe(),
            cursor: state.sequence,
            closed: false,
        })
    }
    pub(crate) fn publish(&self, kind: ChangeKind) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| CoreError::internal("event state poisoned after commit"))?;
        if state.closed {
            return Err(CoreError::internal(
                "write committed after notifications closed",
            ));
        }
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| CoreError::internal("write committed but event sequence exhausted"))?;
        state.sequence = sequence;
        state.events.push_back(ChangeEvent { sequence, kind });
        if state.events.len() > self.capacity {
            state.events.pop_front();
        }
        self.signal.send_replace(sequence);
        Ok(())
    }
    pub(crate) fn close(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| CoreError::internal("event state poisoned"))?;
        state.closed = true;
        self.signal.send_replace(state.sequence);
        Ok(())
    }
}
/// A bounded invalidation stream. Subscribe before loading a page; if lagged,
/// reload relevant data and continue from through_sequence. No image payloads.
pub struct Subscription {
    hub: Arc<EventHub>,
    receiver: watch::Receiver<u64>,
    cursor: u64,
    closed: bool,
}
impl Subscription {
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.hub.subscribers.fetch_sub(1, Ordering::Relaxed);
        }
    }
    pub async fn next(&mut self) -> Result<Notification> {
        loop {
            if self.closed {
                return Ok(Notification::Closed);
            }
            let notification = {
                let state = self
                    .hub
                    .state
                    .lock()
                    .map_err(|_| CoreError::internal("event state poisoned"))?;
                if state
                    .events
                    .front()
                    .is_some_and(|event| self.cursor.saturating_add(1) < event.sequence)
                {
                    self.cursor = state.sequence;
                    Some(Notification::ReloadRequired {
                        through_sequence: state.sequence,
                    })
                } else if let Some(event) = state
                    .events
                    .iter()
                    .find(|event| event.sequence > self.cursor)
                {
                    self.cursor = event.sequence;
                    Some(Notification::Changed(event.clone()))
                } else if state.closed {
                    Some(Notification::Closed)
                } else {
                    None
                }
            };
            if let Some(notification) = notification {
                if notification == Notification::Closed {
                    self.close();
                }
                return Ok(notification);
            }
            if self.receiver.changed().await.is_err() {
                self.close();
                return Ok(Notification::Closed);
            }
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.close();
    }
}
