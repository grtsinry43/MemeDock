mod client;
mod importing;
mod preview;
#[cfg(test)]
mod tests;
use crate::{
    CoreError, ErrorCode, Library, Result,
    runtime::Services,
    tasks::{Priority, Task, TaskControl, scheduler::Lane},
};
pub use importing::{
    TelegramImportController, TelegramImportItem, TelegramImportOutcome, TelegramImportReport,
    TelegramImportTask,
};
use memedock_domain::{
    change::{Operation, OperationKind},
    collection::Collection,
    identity::{CollectionId, LibraryId, OperationId},
    ordering::SortKey,
    source::{SourceImportState, SourceItemId, SourcePack, TelegramPackName},
    tag::Name,
    version::TimestampMs,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelegramFormat {
    Static,
    Tgs,
    Webm,
}
#[derive(Clone, Debug)]
pub struct TelegramSticker {
    pub id: SourceItemId,
    pub emoji: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: TelegramFormat,
    pub state: SourceImportState,
    pub has_preview: bool,
}
pub struct TelegramPack {
    pub(crate) name: TelegramPackName,
    pub(crate) title: String,
    pub(crate) stickers: Vec<TelegramSticker>,
    library: LibraryId,
    client: client::Client,
    remote: Vec<RemoteSticker>,
}
impl TelegramPack {
    pub fn name(&self) -> &TelegramPackName {
        &self.name
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn stickers(&self) -> &[TelegramSticker] {
        &self.stickers
    }
    fn validate_library(&self, library: &Library) -> Result<()> {
        if self.library != library.identity().library_id {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "telegram session belongs to another library",
            ));
        }
        Ok(())
    }
    fn remote(&self, id: &SourceItemId) -> Result<&RemoteSticker> {
        self.remote
            .iter()
            .find(|s| s.file_unique_id == id.as_str())
            .ok_or_else(|| {
                CoreError::new(ErrorCode::InvalidInput, "sticker is not in selected pack")
            })
    }
}
#[derive(Deserialize)]
struct RemotePack {
    name: String,
    title: String,
    stickers: Vec<RemoteSticker>,
}
#[derive(Deserialize)]
struct RemoteThumbnail {
    file_id: String,
    file_unique_id: String,
}
#[derive(Deserialize)]
struct RemoteSticker {
    file_id: String,
    file_unique_id: String,
    width: u32,
    height: u32,
    is_animated: bool,
    is_video: bool,
    emoji: Option<String>,
    thumbnail: Option<RemoteThumbnail>,
}
impl RemoteSticker {
    fn format(&self) -> TelegramFormat {
        if self.is_animated {
            TelegramFormat::Tgs
        } else if self.is_video {
            TelegramFormat::Webm
        } else {
            TelegramFormat::Static
        }
    }
}
pub(crate) struct SourceCommit {
    pub(crate) pack: TelegramPackName,
    pub(crate) title: String,
    pub(crate) item: SourceItemId,
}
pub(crate) async fn source_collection(
    tx: &mut memedock_storage::transaction::WriteTransaction,
    source: &SourceCommit,
    at: TimestampMs,
    control: &TaskControl,
) -> Result<Collection> {
    if let Some(pack) = tx.source_pack(&source.pack).await?
        && let Some(collection) = tx.collection(pack.collection).await?
        && collection.lifecycle().is_active()
    {
        return Ok(collection);
    }
    let name = Name::new(format!("Telegram-{}", source.title))?;
    let ordered = tx.active_collections().await?;
    let collection = Collection::new(
        CollectionId::new(),
        name.clone(),
        SortKey::between(ordered.last().map(Collection::sort_key), None)?,
        at,
    );
    control.begin_commit()?;
    tx.save_collection(&collection).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::CreateCollection {
            collection_id: collection.id(),
            name,
            before_id: None,
        })?,
        at,
    )
    .await?;
    tx.save_source_pack(&SourcePack {
        name: source.pack.clone(),
        collection: collection.id(),
    })
    .await?;
    Ok(collection)
}
pub(super) async fn import_state(
    services: &Services,
    id: &SourceItemId,
) -> Result<SourceImportState> {
    let Some(item) = services.db.source_item(id).await? else {
        return Ok(SourceImportState::Available);
    };
    let sticker = services
        .db
        .sticker(item.sticker)
        .await?
        .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "source sticker missing"))?;
    if !sticker.lifecycle().is_active() {
        return Ok(SourceImportState::RestoreRequired);
    }
    let local = services.db.local_asset(item.sticker.content_hash()).await?;
    let asset = services
        .db
        .asset(item.sticker.content_hash())
        .await?
        .ok_or_else(|| CoreError::new(ErrorCode::CorruptData, "source asset missing"))?;
    let path = services.blobs.original_path(asset.hash());
    let present = tokio::task::spawn_blocking(move || {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.is_file() && metadata.len() == asset.byte_size().get() as u64
        })
    })
    .await?;
    Ok(
        if present
            && local.is_some_and(|v| v.blob_status() == memedock_domain::local::BlobStatus::Ready)
        {
            SourceImportState::Imported
        } else {
            SourceImportState::OriginalMissing
        },
    )
}
pub fn parse_pack(value: &str) -> Result<TelegramPackName> {
    let value = value.trim();
    if !value.contains("://") {
        return Ok(TelegramPackName::new(value.to_owned())?);
    }
    let url = reqwest::Url::parse(value)
        .map_err(|_| CoreError::new(ErrorCode::InvalidInput, "invalid sticker pack link"))?;
    let name = if url.scheme() == "https"
        && matches!(url.host_str(), Some("t.me" | "telegram.me"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
    {
        url.path()
            .strip_prefix("/addstickers/")
            .ok_or_else(|| CoreError::new(ErrorCode::InvalidInput, "not a sticker pack link"))?
            .trim_end_matches('/')
            .to_owned()
    } else if url.scheme() == "tg" && url.host_str() == Some("addstickers") {
        url.query_pairs()
            .find(|(k, _)| k == "set")
            .map(|(_, v)| v.into_owned())
            .ok_or_else(|| CoreError::new(ErrorCode::InvalidInput, "sticker pack name missing"))?
    } else {
        return Err(CoreError::new(
            ErrorCode::InvalidInput,
            "unsupported sticker pack link",
        ));
    };
    Ok(TelegramPackName::new(name)?)
}
impl Library {
    pub fn telegram_pack(
        &self,
        token: String,
        name_or_link: String,
    ) -> Result<Task<Arc<TelegramPack>>> {
        let name = parse_pack(&name_or_link)?;
        self.load_telegram_pack(token, name, None)
    }
    fn load_telegram_pack(
        &self,
        token: String,
        name: TelegramPackName,
        endpoint: Option<reqwest::Url>,
    ) -> Result<Task<Arc<TelegramPack>>> {
        let library = self.identity().library_id;
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                let client = tokio::task::spawn_blocking(move || match endpoint {
                    Some(base) => client::Client::with_base(token, base),
                    None => client::Client::new(token),
                })
                .await??;
                let remote: RemotePack = client
                    .call(
                        "getStickerSet",
                        serde_json::json!({"name": name.as_str()}),
                        &control,
                    )
                    .await?;
                let returned_name = TelegramPackName::new(remote.name)?;
                if returned_name != name
                    || remote.title.is_empty()
                    || remote.title.len() > 512
                    || remote.stickers.is_empty()
                    || remote.stickers.len() > 1000
                {
                    return Err(CoreError::new(
                        ErrorCode::CorruptData,
                        "invalid telegram sticker pack",
                    ));
                }
                // Validate the generated collection name before any files are downloaded.
                Name::new(format!("Telegram-{}", remote.title))?;
                let mut stickers = Vec::new();
                let mut seen = std::collections::BTreeSet::new();
                for sticker in &remote.stickers {
                    control.check()?;
                    let id = SourceItemId::new(sticker.file_unique_id.clone())?;
                    if !seen.insert(id.clone())
                        || sticker.width == 0
                        || sticker.height == 0
                        || sticker.width > 512
                        || sticker.height > 512
                        || sticker.is_animated && sticker.is_video
                        || sticker.file_id.len() > 1024
                    {
                        return Err(CoreError::new(
                            ErrorCode::CorruptData,
                            "invalid telegram sticker",
                        ));
                    }
                    stickers.push(TelegramSticker {
                        state: import_state(&services, &id).await?,
                        id,
                        emoji: sticker.emoji.clone(),
                        width: sticker.width,
                        height: sticker.height,
                        format: sticker.format(),
                        has_preview: sticker.thumbnail.is_some(),
                    });
                }
                Ok(Arc::new(TelegramPack {
                    name: returned_name,
                    title: remote.title,
                    stickers,
                    library,
                    client,
                    remote: remote.stickers,
                }))
            },
        )
    }
}
