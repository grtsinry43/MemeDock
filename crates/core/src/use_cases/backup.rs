use crate::{
    ArchiveInput, BackupFile, Library, Result, archive,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_storage::{
    LibraryDatabase,
    files::archive::{archive_directory, remove_archive_file},
};
use std::sync::Arc;

impl Library {
    pub fn create_backup(&self) -> Result<Task<BackupFile>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            |services, control| async move {
                let root = services.config.data_dir.clone();
                let directory =
                    tokio::task::spawn_blocking(move || archive_directory(&root)).await??;
                let cleanup_directory = directory.clone();
                let result = async {
                    let snapshot = directory.join("snapshot.sqlite");
                    {
                        let _permit = services
                            .write_permit
                            .acquire()
                            .await
                            .map_err(|_| crate::CoreError::internal("write service closed"))?;
                        control.check()?;
                        control.stage("creating_backup_snapshot");
                        services.db.snapshot(&snapshot).await?;
                    }
                    let snapshot_db = LibraryDatabase::open(&snapshot).await?;
                    let data = snapshot_db
                        .archive_data_bounded(services.config.limits.max_archive_metadata_bytes)
                        .await;
                    snapshot_db.close().await?;
                    let data = data?;
                    let at = crate::writes::now()?;
                    let path = directory.join("backup.memedock");
                    let worker = services.clone();
                    let check = control.clone();
                    let output = tokio::task::spawn_blocking(move || {
                        archive::format::write(
                            &path,
                            &data,
                            &worker.blobs,
                            &worker.config.limits,
                            at,
                            &check,
                        )
                    })
                    .await??;
                    let root = services.config.data_dir.clone();
                    tokio::task::spawn_blocking(move || remove_archive_file(&root, &snapshot))
                        .await??;
                    Ok(output)
                }
                .await;
                if result.is_err() {
                    let root = services.config.data_dir.clone();
                    tokio::task::spawn_blocking(move || {
                        memedock_storage::files::archive::remove_archive_directory(
                            &root,
                            &cleanup_directory,
                        )
                    })
                    .await??;
                }
                result
            },
        )
    }
    pub fn create_archive_input(&self) -> Result<Task<ArchiveInput>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            |services, control| async move {
                control.check()?;
                tokio::task::spawn_blocking(move || {
                    let root = services.config.data_dir.clone();
                    let path = archive_directory(&root)?.join("input.memedock");
                    std::fs::File::options()
                        .write(true)
                        .create_new(true)
                        .open(&path)?
                        .sync_all()?;
                    Ok(ArchiveInput { path, root })
                })
                .await?
            },
        )
    }
    pub fn discard_archive_input(&self, input: ArchiveInput) -> Result<Task<()>> {
        self.discard_archive_path(input.root, input.path)
    }
    pub fn discard_backup(&self, backup: BackupFile) -> Result<Task<()>> {
        self.discard_archive_path(self.data_dir().to_owned(), backup.path)
    }
    pub fn discard_prepared_archive(
        &self,
        prepared: Arc<crate::PreparedArchive>,
    ) -> Result<Task<()>> {
        self.discard_archive_path(prepared.root.clone(), prepared.path.clone())
    }
    fn discard_archive_path(
        &self,
        root: std::path::PathBuf,
        path: std::path::PathBuf,
    ) -> Result<Task<()>> {
        if root != self.data_dir() {
            return Err(crate::CoreError::new(
                crate::ErrorCode::InvalidInput,
                "foreign archive",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |_, _| async move {
                tokio::task::spawn_blocking(move || remove_archive_file(&root, &path))
                    .await?
                    .map_err(Into::into)
            },
        )
    }
}
