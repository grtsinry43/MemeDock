use crate::i18n::Key;
use memedock_core::sync::SyncStatus;

#[derive(Clone, Default)]
pub struct State {
    pub status: SyncStatus,
    pub busy: bool,
    pub loaded: bool,
    pub credential: bool,
    pub file_credential: bool,
    pub error: Option<Key>,
    pub terminal: bool,
    pub suspended: bool,
}
impl State {
    pub fn automatic(&self) -> bool {
        self.loaded
            && self.credential
            && self.status.connected
            && !self.status.paused
            && !self.suspended
            && !self.terminal
    }
    pub fn needs_authorization(&self) -> bool {
        !self.credential || self.status.error.as_deref() == Some("unauthorized")
    }
}

pub(super) enum Command {
    Connect { origin: String, token: String },
    Synchronize,
    Pause(bool),
    Disconnect,
}
