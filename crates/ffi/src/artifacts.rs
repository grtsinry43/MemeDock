use crate::{BridgeError, ErrorCode, Result};
use std::sync::Arc;

#[derive(Clone, Debug, uniffi::Record)]
pub struct ExportArtifact {
    pub id: String,
    pub path: String,
    pub mime: String,
    pub file_name: String,
    pub byte_size: u64,
    pub animated: bool,
    pub retained_until: i64,
}
impl TryFrom<memedock_core::ExportArtifact> for ExportArtifact {
    type Error = BridgeError;
    fn try_from(v: memedock_core::ExportArtifact) -> Result<Self> {
        Ok(Self {
            id: v.id.to_string(),
            path: v.path.into_os_string().into_string().map_err(|_| {
                BridgeError::new(ErrorCode::InvalidInput, "artifact path is not UTF-8")
            })?,
            mime: v.mime,
            file_name: v.file_name,
            byte_size: v.byte_size,
            animated: v.animated,
            retained_until: v.retained_until,
        })
    }
}
#[derive(uniffi::Object)]
pub struct ArtifactLeaseHandle {
    pub(crate) inner: Arc<memedock_core::ArtifactLease>,
}
impl ArtifactLeaseHandle {
    pub(crate) fn new(inner: Arc<memedock_core::ArtifactLease>) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}
#[uniffi::export]
impl ArtifactLeaseHandle {
    pub fn metadata(&self) -> Result<ExportArtifact> {
        self.inner.metadata().clone().try_into()
    }
}
