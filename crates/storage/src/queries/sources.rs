use crate::{LibraryDatabase, Result, transaction::WriteTransaction};
use memedock_domain::source::{SourceItem, SourceItemId, SourcePack, TelegramPackName};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

async fn item(db: &impl ConnectionTrait, id: &SourceItemId) -> Result<Option<SourceItem>> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT sticker_id FROM telegram_items WHERE unique_id=?",
        [id.as_str().into()],
    ))
    .await?
    .map(|row| {
        Ok(SourceItem {
            id: id.clone(),
            sticker: row.try_get::<String>("", "sticker_id")?.parse()?,
        })
    })
    .transpose()
}
async fn pack(db: &impl ConnectionTrait, name: &TelegramPackName) -> Result<Option<SourcePack>> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT collection_id FROM telegram_packs WHERE name=?",
        [name.as_str().into()],
    ))
    .await?
    .map(|row| {
        Ok(SourcePack {
            name: name.clone(),
            collection: row.try_get::<String>("", "collection_id")?.parse()?,
        })
    })
    .transpose()
}
impl LibraryDatabase {
    pub async fn source_item(&self, id: &SourceItemId) -> Result<Option<SourceItem>> {
        item(&self.connection, id).await
    }
    pub async fn source_pack(&self, name: &TelegramPackName) -> Result<Option<SourcePack>> {
        pack(&self.connection, name).await
    }
}
impl WriteTransaction {
    pub async fn source_item(&self, id: &SourceItemId) -> Result<Option<SourceItem>> {
        item(&self.inner, id).await
    }
    pub async fn source_pack(&self, name: &TelegramPackName) -> Result<Option<SourcePack>> {
        pack(&self.inner, name).await
    }
    pub async fn save_source_item(&mut self, value: &SourceItem) -> Result<()> {
        let result = self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO telegram_items(unique_id,sticker_id) VALUES(?,?) ON CONFLICT(unique_id) DO UPDATE SET sticker_id=excluded.sticker_id",
            [value.id.as_str().into(), value.sticker.to_string().into()])).await.map(|_| ()).map_err(Into::into);
        self.failed |= result.is_err();
        result
    }
    pub async fn save_source_pack(&mut self, value: &SourcePack) -> Result<()> {
        let result = self.inner.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO telegram_packs(name,collection_id) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET collection_id=excluded.collection_id",
            [value.name.as_str().into(), value.collection.to_string().into()])).await.map(|_| ()).map_err(Into::into);
        self.failed |= result.is_err();
        result
    }
}
