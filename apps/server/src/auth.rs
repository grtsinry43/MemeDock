use crate::{error::ApiError, state::AppState};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use memedock_api_contracts::ApiErrorKind;
use memedock_storage::server::AdminAccount;
use sha2::{Digest, Sha256};
use tower_sessions::Session;

pub const AUTH_VERSION: &str = "authVersion";
/// Every request uses a live device row; browser sessions are never accepted here.
pub async fn require_device(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<memedock_domain::identity::DeviceId, ApiError> {
    use subtle::ConstantTimeEq;
    let invalid = || ApiError(ApiErrorKind::InvalidCredential);
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(invalid)?;
    if token.len() != 105 {
        return Err(invalid());
    }
    let id = token
        .strip_prefix("mdk_")
        .and_then(|v| v.split_once('_'))
        .map(|v| v.0)
        .ok_or_else(invalid)?;
    let device = id.parse().map_err(|_| invalid())?;
    let row = state.db.device(id).await?.ok_or_else(invalid)?;
    if row.revoked_at.is_some()
        || !bool::from(digest(token).as_bytes().ct_eq(row.token_hash.as_bytes()))
    {
        return Err(invalid());
    }
    if !state
        .db
        .mark_device_seen(id, time::OffsetDateTime::now_utc().unix_timestamp())
        .await?
    {
        return Err(invalid());
    }
    Ok(device)
}
pub fn digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn random_secret() -> Result<String, ApiError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| ApiError(ApiErrorKind::ServerError))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn valid_password(password: &str) -> bool {
    (12..=1024).contains(&password.len())
}
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    if !valid_password(password) {
        return Err(ApiError(ApiErrorKind::ParamsError));
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| ApiError(ApiErrorKind::ServerError))
}
pub async fn verify_password(
    state: &AppState,
    password: String,
    hash: String,
) -> Result<bool, ApiError> {
    let permit = state
        .password_budget
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError(ApiErrorKind::TooManyRequests))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let parsed = PasswordHash::new(&hash).map_err(|_| ApiError(ApiErrorKind::ServerError))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| ApiError(ApiErrorKind::ServerError))?
}
pub async fn require_admin(state: &AppState, session: &Session) -> Result<AdminAccount, ApiError> {
    let version: String = session
        .get(AUTH_VERSION)
        .await?
        .ok_or(ApiError(ApiErrorKind::NotLogin))?;
    let account = state
        .db
        .admin()
        .await?
        .ok_or(ApiError(ApiErrorKind::NotLogin))?;
    if account.auth_version != version {
        session.flush().await?;
        return Err(ApiError(ApiErrorKind::NotLogin));
    }
    Ok(account)
}
