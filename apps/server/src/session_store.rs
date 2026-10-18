use crate::auth::{AUTH_VERSION, digest};
use async_trait::async_trait;
use memedock_storage::server::ServerDatabase;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store::{self, Error},
};

#[derive(Clone, Debug)]
pub struct SqliteSessions(pub ServerDatabase);

#[async_trait]
impl SessionStore for SqliteSessions {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let version = record
            .data
            .get(AUTH_VERSION)
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Backend("session has no account version".into()))?;
        let payload =
            serde_json::to_string(&record.data).map_err(|e| Error::Encode(e.to_string()))?;
        for _ in 0..3 {
            if self
                .0
                .create_session(
                    &digest(&record.id.to_string()),
                    &payload,
                    record.expiry_date.unix_timestamp(),
                    version,
                )
                .await
                .map_err(backend)?
            {
                return Ok(());
            }
            record.id = Id::default();
        }
        Err(Error::Backend(
            "session creation conflicted with account reset or ID collision".into(),
        ))
    }
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let payload =
            serde_json::to_string(&record.data).map_err(|e| Error::Encode(e.to_string()))?;
        self.0
            .save_session(
                &digest(&record.id.to_string()),
                &payload,
                record.expiry_date.unix_timestamp(),
            )
            .await
            .map_err(backend)
    }
    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        self.0
            .load_session(
                &digest(&id.to_string()),
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .map_err(backend)?
            .map(|row| {
                Ok(Record {
                    id: *id,
                    data: serde_json::from_str(&row.payload)
                        .map_err(|e| Error::Decode(e.to_string()))?,
                    expiry_date: time::OffsetDateTime::from_unix_timestamp(row.expires_at)
                        .map_err(|e| Error::Decode(e.to_string()))?,
                })
            })
            .transpose()
    }
    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        self.0
            .delete_session(&digest(&id.to_string()))
            .await
            .map_err(backend)
    }
}
fn backend(error: memedock_storage::StorageError) -> Error {
    Error::Backend(error.to_string())
}
