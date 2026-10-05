use crate::{
    CoreError, Library, Result,
    runtime::Services,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use std::sync::Arc;

pub(crate) async fn cleanup(services: &Arc<Services>, now: i64) -> Result<u64> {
    let mut after = None;
    let mut removed = 0;
    loop {
        let records = services.db.artifact_page(after).await?;
        if records.is_empty() {
            break;
        }
        after = records.last().map(|r| r.id);
        for mut record in records {
            if record.retained_until > now || services.artifacts.active(record.id)? {
                continue;
            }
            record.deleting = true;
            services.db.save_artifact(&record).await?;
            let store = services.artifacts.store.clone();
            let candidate = record.clone();
            tokio::task::spawn_blocking(move || store.remove(&candidate)).await??;
            services.db.remove_artifact(record.id).await?;
            removed += 1;
        }
    }
    let older_than = now.saturating_sub(crate::artifacts::MIN_RETENTION_MS);
    let mut cursor: Option<String> = None;
    loop {
        let store = services.artifacts.store.clone();
        let after = cursor.clone();
        let files =
            tokio::task::spawn_blocking(move || store.orphan_page(after.as_deref(), older_than))
                .await??;
        if files.is_empty() {
            break;
        }
        cursor = files.last().map(|v| v.cursor().to_owned());
        for file in files {
            if services.db.artifact_exists(file.id).await? || services.artifacts.active(file.id)? {
                continue;
            }
            let store = services.artifacts.store.clone();
            tokio::task::spawn_blocking(move || store.remove_orphan(&file)).await??;
            removed += 1;
        }
    }
    Ok(removed)
}
impl Library {
    pub fn clean_export_artifacts(&self) -> Result<Task<u64>> {
        self.submit(
            Lane::Blocking,
            Priority::Background,
            move |services, control| async move {
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                control.begin_commit()?;
                cleanup(&services, crate::writes::now()?.get()).await
            },
        )
    }
}
