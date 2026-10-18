use crate::{config::ServerConfig, error::ApiError};
use memedock_api_contracts::ApiErrorKind;
use memedock_storage::server::ServerDatabase;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    pub db: ServerDatabase,
    pub config: ServerConfig,
    pub password_budget: Arc<Semaphore>,
    pub transfer_budget: Arc<Semaphore>,
    pub thumbnail_requests: Arc<Semaphore>,
    pub thumbnail_budget: Arc<Semaphore>,
    pub asset_write_gate: Arc<Semaphore>,
    login_attempts: Arc<Mutex<VecDeque<Instant>>>,
}
impl AppState {
    pub fn new(db: ServerDatabase, config: ServerConfig) -> Self {
        Self {
            db,
            config,
            password_budget: Arc::new(Semaphore::new(2)),
            transfer_budget: Arc::new(Semaphore::new(2)),
            thumbnail_requests: Arc::new(Semaphore::new(64)),
            thumbnail_budget: Arc::new(Semaphore::new(2)),
            asset_write_gate: Arc::new(Semaphore::new(1)),
            login_attempts: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    pub fn check_login_rate(&self) -> Result<(), ApiError> {
        let mut attempts = self
            .login_attempts
            .lock()
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
        let now = Instant::now();
        while attempts
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            attempts.pop_front();
        }
        if attempts.len() >= 30 {
            return Err(ApiError(ApiErrorKind::TooManyRequests));
        }
        attempts.push_back(now);
        Ok(())
    }
}
