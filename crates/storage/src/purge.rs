use crate::{LibraryDatabase, Result};
use memedock_domain::{identity::ContentHash, version::Generation};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

impl LibraryDatabase {
    pub async fn purge_jobs(&self) -> Result<Vec<(ContentHash, Generation)>> {
        self.connection
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT hash,generation FROM purge_cleanup ORDER BY hash LIMIT 128".to_owned(),
            ))
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<String>("", "hash")?.parse()?,
                    Generation::new(row.try_get("", "generation")?)?,
                ))
            })
            .collect()
    }
    pub async fn finish_purge_job(&self, hash: ContentHash, generation: Generation) -> Result<()> {
        self.connection
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM purge_cleanup WHERE hash=? AND generation=?",
                [hash.to_string().into(), generation.get().into()],
            ))
            .await?;
        Ok(())
    }
}
