//! Native integration fixture, not a substitute for the production core.
//! Exercises suspended futures and deterministic shutdown without media or IO.
use std::{
    fmt,
    future::poll_fn,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    task::{Poll, Waker},
};

uniffi::setup_scaffolding!();

static LIVE_PROBES: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, uniffi::Error)]
pub enum ProbeError {
    Cancelled,
    Closed,
    Busy,
    Internal,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ProbeError {}

#[derive(uniffi::Record)]
pub struct Payload {
    pub text: String,
    pub bytes: Vec<u8>,
    pub number: u64,
    pub labels: Vec<String>,
    pub optional: Option<String>,
}

#[uniffi::export]
pub fn round_trip(payload: Payload) -> Payload {
    payload
}

#[uniffi::export]
pub async fn round_trip_async(payload: Payload) -> Payload {
    payload
}

#[uniffi::export]
pub fn live_probe_count() -> u32 {
    LIVE_PROBES.load(Ordering::SeqCst)
}

#[derive(Default)]
struct State {
    cancelled: bool,
    closed: bool,
    waiting: bool,
    waker: Option<Waker>,
}

#[derive(uniffi::Object)]
pub struct Probe {
    state: Mutex<State>,
}

#[uniffi::export]
pub fn new_probe() -> Arc<Probe> {
    LIVE_PROBES.fetch_add(1, Ordering::SeqCst);
    Arc::new(Probe {
        state: Mutex::new(State::default()),
    })
}

#[uniffi::export]
pub fn retain_probe(probe: Arc<Probe>) -> Arc<Probe> {
    probe
}

// Dropping a foreign future must also release its single-reader reservation.
struct Reader<'a>(&'a Probe);

impl Drop for Reader<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
        state.waiting = false;
        state.waker = None;
    }
}

impl Probe {
    fn signal(&self, close: bool) -> Result<(), ProbeError> {
        let waker = {
            let mut state = self.state.lock().map_err(|_| ProbeError::Internal)?;
            if close {
                state.closed = true;
            } else {
                state.cancelled = true;
            }
            state.waker.take()
        };
        // Foreign runtime callbacks must not run under the state mutex.
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
}

#[uniffi::export]
impl Probe {
    pub fn cancel(&self) -> Result<(), ProbeError> {
        self.signal(false)
    }

    pub fn close(&self) -> Result<(), ProbeError> {
        self.signal(true)
    }

    pub fn has_waiter(&self) -> Result<bool, ProbeError> {
        Ok(self.state.lock().map_err(|_| ProbeError::Internal)?.waiting)
    }

    pub async fn wait_until_cancelled(&self) -> Result<(), ProbeError> {
        {
            let mut state = self.state.lock().map_err(|_| ProbeError::Internal)?;
            if state.waiting {
                return Err(ProbeError::Busy);
            }
            state.waiting = true;
        }
        let _reader = Reader(self);
        poll_fn(|cx| {
            let Ok(mut state) = self.state.lock() else {
                return Poll::Ready(Err(ProbeError::Internal));
            };
            if state.closed {
                return Poll::Ready(Err(ProbeError::Closed));
            }
            if state.cancelled {
                return Poll::Ready(Err(ProbeError::Cancelled));
            }
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        LIVE_PROBES.fetch_sub(1, Ordering::SeqCst);
    }
}
