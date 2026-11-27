use crate::{
    ArtifactLease, CoreError, ErrorCode, Library, Result,
    artifacts::MIN_RETENTION_MS,
    creator::{CreatedAnimation, CreatorVideo, MAX_VIDEO_BYTES, VideoEdit},
    runtime::Services,
    tasks::{Priority, Task, TaskControl, scheduler::Lane},
};
use memedock_domain::{
    asset::ImageFormat,
    export::{AnimationPolicy, ExportOptions, ExportPreset, GifSizeLimit},
    identity::{ContentHash, OperationId},
};
use memedock_storage::{artifacts::ArtifactRecord, files::StagedFile};
use std::{fs::File, path::PathBuf, sync::Arc, time::Instant};

async fn available(services: &Arc<Services>, control: &TaskControl) -> Result<u64> {
    control.check()?;
    super::artifact_maintenance::cleanup(services, crate::writes::now()?.get()).await?;
    let store = services.artifacts.store.clone();
    let used = tokio::task::spawn_blocking(move || store.bytes_used()).await??;
    let available = services
        .config
        .limits
        .export_budget_bytes
        .saturating_sub(used);
    if available == 0 {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "creation output quota exceeded",
        ));
    }
    Ok(available)
}

async fn publish(
    services: Arc<Services>,
    control: Arc<TaskControl>,
    staged: StagedFile,
    source: Option<ContentHash>,
    format: ImageFormat,
    animated: bool,
    recipe: String,
) -> Result<Arc<ArtifactLease>> {
    let hash = source.unwrap_or(staged.hash());
    let association = services.db.asset(hash).await?.map(|v| v.hash());
    if let Some(mut cached) = services.db.artifact_for_recipe(hash, &recipe).await? {
        let store = services.artifacts.store.clone();
        let candidate = cached.clone();
        let check = control.clone();
        let verified =
            tokio::task::spawn_blocking(move || store.verify(&candidate, || check.is_cancelled()))
                .await?;
        if let Err(error) = control.check() {
            staged.discard()?;
            return Err(error);
        }
        if verified.is_ok()
            && !cached.deleting
            && cached.format == format
            && cached.animated == animated
        {
            tokio::task::spawn_blocking(move || staged.discard()).await??;
            control.begin_commit()?;
            cached.source_asset_hash = association.or(cached.source_asset_hash);
            cached.retained_until = cached.retained_until.max(
                crate::writes::now()?
                    .get()
                    .checked_add(MIN_RETENTION_MS)
                    .ok_or_else(|| CoreError::internal("retention overflow"))?,
            );
            services.db.save_artifact(&cached).await?;
            return services.artifacts.lease(cached);
        }
        if services.artifacts.active(cached.id)? {
            staged.discard()?;
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "active creation output unavailable",
            ));
        }
        cached.deleting = true;
        services.db.save_artifact(&cached).await?;
    }
    let record = ArtifactRecord {
        id: OperationId::new(),
        source_hash: hash,
        source_asset_hash: association,
        output_hash: staged.hash(),
        byte_size: staged.byte_size(),
        recipe,
        format,
        animated,
        retained_until: crate::writes::now()?
            .get()
            .checked_add(MIN_RETENTION_MS)
            .ok_or_else(|| CoreError::internal("retention overflow"))?,
        deleting: false,
    };
    let store = services.artifacts.store.clone();
    let candidate = record.clone();
    let check = control.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let result = staged
            .open_read()
            .map_err(CoreError::from)
            .and_then(|file| {
                store
                    .publish(&candidate, file, || check.is_cancelled())
                    .map_err(CoreError::from)
            });
        staged.discard()?;
        result?;
        Ok(())
    })
    .await??;
    control.begin_commit()?;
    services.db.save_artifact(&record).await?;
    services.artifacts.lease(record)
}

impl Library {
    pub fn open_creator_video(&self, path: PathBuf) -> Result<Task<Arc<CreatorVideo>>> {
        let library_id = self.identity().library_id;
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                let allowance = services.image_budget.acquire().await?;
                tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    control.stage("copying_video");
                    let mut input = File::open(path)?;
                    let metadata = input.metadata()?;
                    if !metadata.is_file() || metadata.len() > MAX_VIDEO_BYTES {
                        return Err(CoreError::new(
                            ErrorCode::ResourceLimit,
                            "video must be a local file up to 2 GiB",
                        ));
                    }
                    let staged = services
                        .blobs
                        .stage_from(&mut input, MAX_VIDEO_BYTES, || control.is_cancelled())?;
                    control.stage("inspecting_video");
                    let result =
                        crate::images::video::inspect(&staged, &services.config.limits, &control);
                    match result.and_then(|info| {
                        control.begin_commit()?;
                        Ok(info)
                    }) {
                        Ok(info) => Ok(Arc::new(CreatorVideo {
                            staged,
                            library_id,
                            info,
                        })),
                        Err(error) => {
                            staged.discard()?;
                            Err(error)
                        }
                    }
                })
                .await?
            },
        )
    }
    pub fn discard_creator_video(&self, video: Arc<CreatorVideo>) -> Result<Task<()>> {
        if video.library_id != self.identity().library_id {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "video belongs to another library",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |_, _| async move {
                tokio::task::spawn_blocking(move || {
                    let video = Arc::try_unwrap(video)
                        .map_err(|_| CoreError::new(ErrorCode::Busy, "video still in use"))?;
                    Ok(video.staged.discard()?)
                })
                .await?
            },
        )
    }
    pub fn create_video_webp(
        &self,
        video: Arc<CreatorVideo>,
        edit: VideoEdit,
    ) -> Result<Task<CreatedAnimation>> {
        if video.library_id != self.identity().library_id || edit.end_ms() > video.info.duration_ms
        {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "selection outside video",
            ));
        }
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
                let bytes = available(&services, &control).await?;
                let allowance = services.image_budget.acquire().await?;
                let worker = services.clone();
                let check = control.clone();
                let encoded = tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    crate::images::video_webp::encode(
                        &video.staged,
                        edit,
                        &worker.blobs,
                        &worker.config.limits,
                        bytes,
                        &check,
                    )
                })
                .await??;
                let (width, height) = (encoded.width, encoded.height);
                let artifact = publish(
                    services.clone(),
                    control,
                    encoded.staged,
                    None,
                    ImageFormat::WebP,
                    true,
                    "creator-webp-v1".into(),
                )
                .await?;
                Ok(CreatedAnimation {
                    artifact,
                    width,
                    height,
                    duration_ms: edit.duration_ms(),
                    frame_rate: f64::from(edit.fps()),
                })
            },
        )
    }
    pub fn creator_preview(
        &self,
        video: Arc<CreatorVideo>,
        time_ms: u64,
    ) -> Result<Task<Arc<ArtifactLease>>> {
        if video.library_id != self.identity().library_id || time_ms >= video.info.duration_ms {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "preview outside video",
            ));
        }
        self.submit(
            Lane::Blocking,
            Priority::Visible,
            move |services, control| async move {
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                let bytes = available(&services, &control).await?;
                let allowance = services.image_budget.acquire().await?;
                let worker = services.clone();
                let check = control.clone();
                let staged = tokio::task::spawn_blocking(move || -> Result<StagedFile> {
                    let _allowance = allowance;
                    let started = Instant::now();
                    let mut reader = crate::images::video::Reader::open(
                        &video.staged,
                        &worker.config.limits,
                        &check,
                        started,
                    )?;
                    let edit =
                        VideoEdit::new(time_ms, time_ms + 1, Default::default(), 192, 1, 80)?;
                    let pending = worker.blobs.create_staging()?;
                    let result = (|| {
                        reader.render(edit, &check, started, |rgba, w, h, _, _| {
                            use image::ImageEncoder;
                            let file = File::options().write(true).open(pending.path())?;
                            let mut writer = crate::images::export::BoundedWriter {
                                file,
                                remaining: bytes.min(1024 * 1024),
                                control: &check,
                                exceeded: false,
                            };
                            image::codecs::png::PngEncoder::new(&mut writer)
                                .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
                                .map_err(crate::images::inspect::error)
                        })?;
                        Ok(pending.finish(bytes.min(1024 * 1024), || check.is_cancelled())?)
                    })();
                    if result.is_err() {
                        pending.discard()?;
                    }
                    result
                })
                .await??;
                publish(
                    services.clone(),
                    control,
                    staged,
                    None,
                    ImageFormat::Png,
                    false,
                    "creator-preview-v1".into(),
                )
                .await
            },
        )
    }
    /// Same GIF recipe/cache as library export when no byte target is supplied.
    pub fn creation_gif(
        &self,
        source: Arc<ArtifactLease>,
        limit: Option<GifSizeLimit>,
    ) -> Result<Task<Arc<ArtifactLease>>> {
        self.submit(
            Lane::Blocking,
            Priority::Interactive,
            move |services, control| async move {
                if !Arc::ptr_eq(&source.owner, &services.artifacts)
                    || source.record.format != ImageFormat::WebP
                    || !source.record.animated
                {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput,
                        "creation requires an animated WebP from this session",
                    ));
                }
                let _permit = services
                    .artifacts
                    .permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("artifact manager closed"))?;
                let recipe = limit.map_or_else(
                    || {
                        ExportOptions::for_preset(
                            ExportPreset::CompatibleGif,
                            AnimationPolicy::Preserve,
                        )
                        .recipe()
                    },
                    GifSizeLimit::recipe,
                );
                if let Some(mut record) = services
                    .db
                    .artifact_for_recipe(source.record.output_hash, &recipe)
                    .await?
                {
                    let store = services.artifacts.store.clone();
                    let candidate = record.clone();
                    let check = control.clone();
                    let limits = services.config.limits.clone();
                    let verified = tokio::task::spawn_blocking(move || -> Result<()> {
                        store.verify(&candidate, || check.is_cancelled())?;
                        if candidate.format != ImageFormat::Gif
                            || limit.is_some_and(|limit| candidate.byte_size > limit.bytes())
                        {
                            return Err(CoreError::new(
                                ErrorCode::CorruptData,
                                "GIF cache metadata mismatch",
                            ));
                        }
                        let animated = crate::images::gif_export::verify_cached(
                            File::open(store.path(&candidate))?,
                            &limits,
                            &check,
                            Instant::now(),
                        )?;
                        if animated != candidate.animated {
                            return Err(CoreError::new(
                                ErrorCode::CorruptData,
                                "GIF cache animation mismatch",
                            ));
                        }
                        Ok(())
                    })
                    .await?;
                    control.check()?;
                    if verified.is_ok() && !record.deleting {
                        control.begin_commit()?;
                        record.retained_until = record.retained_until.max(
                            crate::writes::now()?
                                .get()
                                .checked_add(MIN_RETENTION_MS)
                                .ok_or_else(|| CoreError::internal("retention overflow"))?,
                        );
                        services.db.save_artifact(&record).await?;
                        return services.artifacts.lease(record);
                    }
                }
                let bytes = available(&services, &control)
                    .await?
                    .min(limit.map_or(u64::MAX, GifSizeLimit::bytes));
                let allowance = services.image_budget.acquire().await?;
                let worker = services.clone();
                let check = control.clone();
                let input = source.clone();
                let (staged, animated) = tokio::task::spawn_blocking(move || {
                    let _allowance = allowance;
                    worker
                        .artifacts
                        .store
                        .verify(&input.record, || check.is_cancelled())?;
                    let parameters: &[(u32, u32)] = if limit.is_some() {
                        &[(512, 30), (384, 24), (256, 20), (192, 15), (128, 10)]
                    } else {
                        &[(512, 30)]
                    };
                    let started = Instant::now();
                    for (i, (edge, fps)) in parameters.iter().enumerate() {
                        check.check()?;
                        let result = crate::images::gif_export::encode_parameters(
                            File::open(&input.metadata().path)?,
                            &worker.blobs,
                            &worker.config.limits,
                            bytes,
                            &check,
                            (*edge, *fps),
                            started,
                        );
                        match result {
                            Err(error)
                                if limit.is_some()
                                    && error.code() == ErrorCode::ResourceLimit
                                    && i + 1 < parameters.len() =>
                            {
                                continue;
                            }
                            other => return other,
                        }
                    }
                    Err(CoreError::internal("GIF attempt missing"))
                })
                .await??;
                publish(
                    services.clone(),
                    control,
                    staged,
                    Some(source.record.output_hash),
                    ImageFormat::Gif,
                    animated,
                    recipe,
                )
                .await
            },
        )
    }
}
