use crate::{
    CoreError, ErrorCode, Library, Result,
    events::ChangeKind,
    tasks::{Priority, Task, scheduler::Lane},
};
use memedock_domain::{
    change::{NamePatch, Operation, OperationKind},
    identity::{OperationId, TagId},
    tag::{Name, Tag},
    version::Generation,
};

impl Library {
    pub fn create_tag(&self, name: Name) -> Result<Task<Tag>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                if let Some(existing) = tx.active_tag_named(&name.normalized()).await? {
                    return Ok(existing);
                }
                let at = crate::writes::now()?;
                let value = Tag::new(TagId::new(), name.clone(), at);
                control.begin_commit()?;
                tx.save_tag(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::CreateTag {
                        tag_id: value.id(),
                        name,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::TagsChanged)?;
                Ok(value)
            },
        )
    }
}
impl Library {
    pub fn rename_tag(&self, id: TagId, generation: Generation, name: Name) -> Result<Task<Tag>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .tag(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "tag missing"))?;
                value.lifecycle().ensure_active(generation)?;
                if value.name() == &name {
                    return Ok(value);
                }
                let at = crate::writes::now()?;
                let patch = NamePatch::new(name);
                value.patch(generation, &patch, at, value.lifecycle().revision())?;
                control.begin_commit()?;
                tx.save_tag(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::PatchTag {
                        tag_id: id,
                        generation,
                        patch,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::TagsChanged)?;
                Ok(value)
            },
        )
    }
    pub fn delete_tag(&self, id: TagId, generation: Generation) -> Result<Task<Tag>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| async move {
                let _permit = services
                    .write_permit
                    .acquire()
                    .await
                    .map_err(|_| CoreError::internal("write service closed"))?;
                control.check()?;
                let mut tx = services.db.begin_write().await?;
                let mut value = tx
                    .tag(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "tag missing"))?;
                let at = crate::writes::now()?;
                if !value.delete(generation, at, value.lifecycle().revision())? {
                    return Ok(value);
                }
                control.begin_commit()?;
                tx.save_tag(&value).await?;
                tx.append_change(
                    OperationId::new(),
                    Operation::new(OperationKind::DeleteTag {
                        tag_id: id,
                        generation,
                    })?,
                    at,
                )
                .await?;
                tx.commit().await?;
                services.events.publish(ChangeKind::TagsChanged)?;
                Ok(value)
            },
        )
    }
}
