use super::ServerDatabase;
use crate::{Result, StorageError};
use memedock_domain::{asset::Asset, identity::ContentHash};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

impl ServerDatabase {
    /// Retain the entity's permanent-delete marker, but stop advertising removed bytes.
    /// Caller holds the same gate as upload publication and sync mutation.
    pub async fn forget_server_asset(&self, hash: ContentHash) -> Result<()> {
        self.connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM server_assets WHERE hash=?",
                [hash.to_string().into()],
            ))
            .await?;
        Ok(())
    }
    pub async fn server_sticker(
        &self,
        hash: ContentHash,
    ) -> Result<Option<memedock_domain::sticker::Sticker>> {
        self.connection
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT payload FROM server_sync_entities WHERE kind='sticker' AND id=?",
                [hash.to_string().into()],
            ))
            .await?
            .map(|row| super::sync::decode(&row.try_get::<String>("", "payload")?))
            .transpose()
    }
    pub async fn server_purge_jobs(
        &self,
    ) -> Result<Vec<(ContentHash, memedock_domain::version::Generation)>> {
        self.connection
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT hash,generation FROM server_purge_cleanup ORDER BY hash LIMIT 128"
                    .to_owned(),
            ))
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<String>("", "hash")?.parse()?,
                    memedock_domain::version::Generation::new(row.try_get("", "generation")?)?,
                ))
            })
            .collect()
    }
    pub async fn finish_server_purge(
        &self,
        hash: ContentHash,
        generation: memedock_domain::version::Generation,
    ) -> Result<()> {
        self.connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM server_purge_cleanup WHERE hash=? AND generation=?",
                [hash.to_string().into(), generation.get().into()],
            ))
            .await?;
        Ok(())
    }
    pub async fn server_asset(&self, hash: ContentHash) -> Result<Option<Asset>> {
        super::sync::read_asset(&self.connection, hash).await
    }
    /// Bytes must already be verified and durably published. First metadata wins.
    pub async fn register_server_asset(&self, asset: &Asset) -> Result<Asset> {
        self.connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO server_assets(hash,payload) VALUES(?,?) ON CONFLICT(hash) DO NOTHING",
                [
                    asset.hash().to_string().into(),
                    super::sync::encode(asset)?.into(),
                ],
            ))
            .await?;
        self.server_asset(asset.hash())
            .await?
            .ok_or(StorageError::Integrity("published asset missing"))
    }
}
