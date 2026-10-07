use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    images::{preview, thumbnail},
    runtime::Services,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    identity::StickerId,
    local::{LocalAsset, ThumbnailStatus},
};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Debug)]
pub struct Thumbnail {
    pub path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub path: PathBuf,
}
impl Library {
    pub fn request_thumbnail(&self, id: StickerId, priority: Priority) -> Result<Task<Thumbnail>> {
        self.submit(
            Lane::Thumbnail,
            priority,
            move |services, control| async move {
                let _same_image = image_lock(&services, id).await?;
                control.check()?;
                let asset = services
                    .db
                    .asset(id.content_hash())
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "asset not found"))?;
                let cache = services.derived.clone();
                let hash = asset.hash();
                let cached =
                    tokio::task::spawn_blocking(move || thumbnail::cached(&cache, hash)).await??;
                if cached {
                    control.begin_commit()?;
                    update(&services, id, Update::Ready).await?;
                    return Ok(Thumbnail {
                        path: services.derived.thumbnail_path(hash),
                    });
                }
                update(&services, id, Update::Generating).await?;
                let allowance = services.image_budget.acquire().await?;
                let worker = services.clone();
                let cancel = control.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    thumbnail::generate(
                        &worker.blobs,
                        &worker.derived,
                        hash,
                        &worker.config.limits,
                        &cancel,
                    )
                })
                .await
                .map_err(CoreError::from)
                .and_then(|value| value);
                match result {
                    Ok(path) => {
                        // A published cache is recoverable if cancellation wins here.
                        if let Err(error) = control.begin_commit() {
                            update(&services, id, Update::Missing).await?;
                            return Err(error);
                        }
                        update(&services, id, Update::Ready).await?;
                        Ok(Thumbnail { path })
                    }
                    Err(error) => {
                        let state = if error.code() == ErrorCode::Cancelled {
                            Update::Missing
                        } else {
                            Update::Failed(error.code().as_str())
                        };
                        update(&services, id, state).await?;
                        Err(error)
                    }
                }
            },
        )
    }

    pub fn request_preview(&self, id: StickerId) -> Result<Task<Preview>> {
        self.submit(
            Lane::Thumbnail,
            Priority::Visible,
            move |services, control| async move {
                let _same_image = image_lock(&services, id).await?;
                control.check()?;
                let asset = services
                    .db
                    .asset(id.content_hash())
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "asset not found"))?;
                let cache = services.derived.clone();
                let hash = asset.hash();
                let cached =
                    tokio::task::spawn_blocking(move || preview::cached(&cache, hash)).await??;
                if cached {
                    return Ok(Preview {
                        path: services.derived.preview_path(hash),
                    });
                }
                let allowance = services.image_budget.acquire().await?;
                let worker = services.clone();
                let cancel = control.clone();
                let path = tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    preview::generate(
                        &worker.blobs,
                        &worker.derived,
                        hash,
                        &worker.config.limits,
                        &cancel,
                    )
                })
                .await
                .map_err(CoreError::from)
                .and_then(|value| value)?;
                control.begin_commit()?;
                Ok(Preview { path })
            },
        )
    }
}

async fn image_lock(
    services: &Services,
    id: StickerId,
) -> Result<tokio::sync::OwnedSemaphorePermit> {
    let lock = {
        let mut locks = services
            .thumbnail_locks
            .lock()
            .map_err(|_| CoreError::internal("thumbnail registry poisoned"))?;
        locks.retain(|_, value| value.strong_count() > 0);
        match locks.get(&id).and_then(std::sync::Weak::upgrade) {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(tokio::sync::Semaphore::new(1));
                locks.insert(id, Arc::downgrade(&lock));
                lock
            }
        }
    };
    lock.acquire_owned()
        .await
        .map_err(|_| CoreError::internal("thumbnail lock closed"))
}
enum Update {
    Generating,
    Ready,
    Missing,
    Failed(&'static str),
}
async fn update(services: &Services, id: StickerId, state: Update) -> Result<()> {
    let _write = services
        .write_permit
        .acquire()
        .await
        .map_err(|_| CoreError::internal("write service closed"))?;
    let mut tx = services.db.begin_write().await?;
    let mut local = tx
        .local_asset(id.content_hash())
        .await?
        .unwrap_or_else(|| LocalAsset::new(id.content_hash()));
    match state {
        Update::Generating => {
            local.begin_thumbnail()?;
        }
        Update::Ready => {
            if local.thumb_status() == ThumbnailStatus::Ready {
                return Ok(());
            }
            if local.thumb_status() != ThumbnailStatus::Generating {
                local.begin_thumbnail()?;
            }
            local.thumbnail_ready()?;
        }
        Update::Missing => local.evict_thumbnail(),
        Update::Failed(code) => local.thumbnail_failed(code.into()),
    }
    tx.save_local_asset(&local).await?;
    tx.commit().await?;
    services.events.publish(ChangeKind::ThumbnailChanged(id))?;
    Ok(())
}
