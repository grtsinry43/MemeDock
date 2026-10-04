#[derive(Debug, uniffi::Object)]
pub struct QueryCursorHandle {
    pub(crate) inner: memedock_core::QueryCursor,
}
