use crate::{
    ArtifactLease, CoreError, ErrorCode, Library, Result,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::identity::OperationId;
use std::sync::Arc;

impl Library {
    /// Persist before setPrimaryClip. The old clipboard remains protected until
    /// the platform confirms replacement; a crash leaves this pending pin intact.
    pub fn protect_clipboard(&self, lease: Arc<ArtifactLease>) -> Result<Task<OperationId>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                if !Arc::ptr_eq(&lease.owner, &services.artifacts) {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "artifact belongs to another library session",
                    ));
                }
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                let record = services
                    .db
                    .artifact_by_id(lease.record.id)
                    .await?
                    .filter(|r| !r.deleting)
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "artifact unavailable"))?;
                let store = services.artifacts.store.clone();
                let check = control.clone();
                let artifact = record.clone();
                tokio::task::spawn_blocking(move || {
                    store.verify(&artifact, || check.is_cancelled())
                })
                .await??;
                control.begin_commit()?;
                let id = OperationId::new();
                services.db.protect_clipboard(id, record.id).await?;
                Ok(id)
            },
        )
    }
    /// Invoke only after a focused foreground platform clipboard observation.
    /// Lack of permission to read is not evidence that the clipboard is empty.
    pub fn reconcile_clipboard(&self, observed: Option<OperationId>) -> Result<Task<()>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                control.begin_commit()?;
                services.db.reconcile_clipboard(observed).await?;
                Ok(())
            },
        )
    }
    pub fn abort_clipboard(&self, reference: OperationId) -> Result<Task<()>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                control.begin_commit()?;
                services.db.abort_clipboard(reference).await?;
                Ok(())
            },
        )
    }
}
