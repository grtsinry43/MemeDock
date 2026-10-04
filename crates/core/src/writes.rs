//! All callers execute these operations on the scheduler's single write lane.
//! Domain/read/commit work cannot overlap another core write; no async mutex is
//! held across SQL or file IO.
use crate::{
    CoreError, ErrorCode, Result, events::ChangeKind, runtime::Services, tasks::TaskControl,
};
use memedock_domain::{
    identity::StickerId,
    local::{LocalUsage, UsageAction},
    version::TimestampMs,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> Result<TimestampMs> {
    let millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis())
            .map_err(|_| CoreError::internal("timestamp overflow"))?,
        Err(error) => -i64::try_from(error.duration().as_millis())
            .map_err(|_| CoreError::internal("timestamp overflow"))?,
    };
    Ok(TimestampMs::new(millis))
}
pub(crate) async fn record_use(
    services: Arc<Services>,
    control: Arc<TaskControl>,
    id: StickerId,
    action: UsageAction,
) -> Result<LocalUsage> {
    control.check()?;
    let mut tx = services.db.begin_write().await?;
    let sticker = tx
        .sticker(id)
        .await?
        .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker not found"))?;
    if !sticker.lifecycle().is_active() {
        return Err(CoreError::new(
            ErrorCode::EntityDeleted,
            "sticker is deleted",
        ));
    }
    let at = now()?;
    let usage = match tx.local_usage(id).await? {
        Some(mut usage) => {
            usage.record(at, action)?;
            usage
        }
        None => LocalUsage::first_use(id, at),
    };
    // Validate cancellation before persistence; after this point close/cancel
    // waits for the real commit and preserves its successful result.
    control.begin_commit()?;
    tx.save_local_usage(&usage).await?;
    tx.commit().await?;
    services.events.publish(ChangeKind::UsageChanged(id))?;
    // UsageAction denotes a platform-observed attempt, never message delivery.
    // No business/sync log entry is added for device-local usage.
    Ok(usage)
}
pub(crate) async fn checkpoint(
    services: Arc<Services>,
    control: Arc<TaskControl>,
) -> Result<PathBuf> {
    control.check()?;
    let directory = services.config.data_dir.join("checkpoints");
    let path = directory.join(format!("{}.sqlite", uuid::Uuid::now_v7()));
    control.begin_commit()?;
    let output = path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        match std::fs::symlink_metadata(&directory) {
            Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
                return Err(CoreError::new(
                    ErrorCode::InvalidInput,
                    "unsafe checkpoint directory",
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&directory)?;
                std::fs::File::open(&services.config.data_dir)?.sync_all()?;
            }
            Err(e) => return Err(e.into()),
        }
        tokio::runtime::Handle::current()
            .block_on(services.db.snapshot(&output))
            .map_err(CoreError::from)
    })
    .await??;
    Ok(path)
}
