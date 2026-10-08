use crate::*;
use memedock_core::sources::telegram as core;
use memedock_domain::source::{SourceImportState, SourceItemId};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum TelegramFormat {
    Static,
    Tgs,
    Webm,
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum TelegramImportState {
    Available,
    Imported,
    RestoreRequired,
    OriginalMissing,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct TelegramSticker {
    pub id: String,
    pub emoji: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: TelegramFormat,
    pub state: TelegramImportState,
    pub has_preview: bool,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct TelegramPackMetadata {
    pub name: String,
    pub title: String,
    pub stickers: Vec<TelegramSticker>,
}

#[derive(uniffi::Object)]
pub struct TelegramPackHandle {
    inner: Arc<core::TelegramPack>,
}
impl TelegramPackHandle {
    pub(crate) fn new(inner: Arc<core::TelegramPack>) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}
#[uniffi::export]
impl TelegramPackHandle {
    pub fn metadata(&self) -> TelegramPackMetadata {
        TelegramPackMetadata {
            name: self.inner.name().as_str().into(),
            title: self.inner.title().into(),
            stickers: self
                .inner
                .stickers()
                .iter()
                .map(|v| TelegramSticker {
                    id: v.id.as_str().into(),
                    emoji: v.emoji.clone(),
                    width: v.width,
                    height: v.height,
                    has_preview: v.has_preview,
                    format: match v.format {
                        core::TelegramFormat::Static => TelegramFormat::Static,
                        core::TelegramFormat::Tgs => TelegramFormat::Tgs,
                        core::TelegramFormat::Webm => TelegramFormat::Webm,
                    },
                    state: match v.state {
                        SourceImportState::Available => TelegramImportState::Available,
                        SourceImportState::Imported => TelegramImportState::Imported,
                        SourceImportState::RestoreRequired => TelegramImportState::RestoreRequired,
                        SourceImportState::OriginalMissing => TelegramImportState::OriginalMissing,
                    },
                })
                .collect(),
        }
    }
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum TelegramImportOutcome {
    Pending,
    Created,
    Reused,
    RestoreRequired,
    Failed,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct TelegramImportItem {
    pub id: String,
    pub sticker: Option<String>,
    pub outcome: TelegramImportOutcome,
    pub error: Option<ErrorCode>,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct TelegramImportReport {
    pub items: Vec<TelegramImportItem>,
    pub stopped: bool,
}
impl From<core::TelegramImportReport> for TelegramImportReport {
    fn from(value: core::TelegramImportReport) -> Self {
        Self {
            stopped: value.stopped,
            items: value
                .items
                .into_iter()
                .map(|item| {
                    let (outcome, error) = match item.outcome {
                        core::TelegramImportOutcome::Pending => {
                            (TelegramImportOutcome::Pending, None)
                        }
                        core::TelegramImportOutcome::Created => {
                            (TelegramImportOutcome::Created, None)
                        }
                        core::TelegramImportOutcome::Reused => {
                            (TelegramImportOutcome::Reused, None)
                        }
                        core::TelegramImportOutcome::RestoreRequired => {
                            (TelegramImportOutcome::RestoreRequired, None)
                        }
                        core::TelegramImportOutcome::Failed(error) => {
                            (TelegramImportOutcome::Failed, Some(error.into()))
                        }
                    };
                    TelegramImportItem {
                        id: item.id.as_str().into(),
                        sticker: item.sticker.map(|s| s.to_string()),
                        outcome,
                        error,
                    }
                })
                .collect(),
        }
    }
}
#[derive(uniffi::Object)]
pub struct TelegramImportTask {
    inner: Mutex<Option<core::TelegramImportTask>>,
    controller: core::TelegramImportController,
}
#[uniffi::export]
impl TelegramImportTask {
    pub fn cancel(&self) -> Result<()> {
        Ok(self.controller.cancel()?)
    }
    pub fn snapshot(&self) -> TelegramImportReport {
        self.controller.snapshot().into()
    }
    pub async fn await_result(&self) -> Result<TelegramImportReport> {
        let task = self
            .inner
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "telegram import slot poisoned"))?
            .take()
            .ok_or_else(|| {
                BridgeError::new(ErrorCode::Conflict, "telegram import already awaited")
            })?;
        Ok(task.wait().await?.into())
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn telegram_pack(
        &self,
        token: String,
        name_or_link: String,
    ) -> Result<Arc<TelegramPackTask>> {
        Ok(TelegramPackTask::new(
            self.inner.telegram_pack(token, name_or_link)?,
        ))
    }
    pub fn telegram_preview(
        &self,
        pack: Arc<TelegramPackHandle>,
        id: String,
    ) -> Result<Arc<TelegramPreviewTask>> {
        Ok(TelegramPreviewTask::new(self.inner.telegram_preview(
            pack.inner.clone(),
            SourceItemId::new(id)?,
        )?))
    }
    pub fn import_telegram(
        &self,
        pack: Arc<TelegramPackHandle>,
        selected: Vec<String>,
    ) -> Result<Arc<TelegramImportTask>> {
        let inner = self.inner.import_telegram(
            pack.inner.clone(),
            selected
                .into_iter()
                .map(SourceItemId::new)
                .collect::<std::result::Result<_, _>>()?,
        )?;
        Ok(Arc::new(TelegramImportTask {
            controller: inner.controller(),
            inner: Mutex::new(Some(inner)),
        }))
    }
}
