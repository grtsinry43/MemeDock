use crate::ErrorCode;
use std::{fmt, str::FromStr};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(Uuid);
impl TaskId {
    pub(crate) fn new() -> Self {
        Self(Uuid::now_v7())
    }
}
impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for TaskId {
    type Err = crate::CoreError;
    fn from_str(value: &str) -> crate::Result<Self> {
        let id = Uuid::parse_str(value)
            .map_err(|e| crate::CoreError::caused(ErrorCode::InvalidInput, "invalid task ID", e))?;
        if id.get_version_num() != 7 || id.get_variant() != uuid::Variant::RFC4122 {
            return Err(crate::CoreError::new(
                ErrorCode::InvalidInput,
                "task ID must be UUIDv7",
            ));
        }
        Ok(Self(id))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Interactive,
    Visible,
    Background,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Committing,
    Succeeded,
    Failed(ErrorCode),
    Cancelled,
}
impl TaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed(_) | Self::Cancelled)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub status: TaskStatus,
    pub cancellation_requested: bool,
    /// Stages are real milestones, not estimated percentages.
    pub stage: &'static str,
}
