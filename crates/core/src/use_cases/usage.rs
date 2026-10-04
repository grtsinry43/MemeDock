use crate::{
    Library, Result,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use memedock_domain::{
    identity::StickerId,
    local::{LocalUsage, UsageAction},
};
impl Library {
    /// Record an observed platform action (including ShareLaunched), not a
    /// promise that a recipient received or delivered the image.
    pub fn record_use(&self, id: StickerId, action: UsageAction) -> Result<Task<LocalUsage>> {
        self.submit(
            Lane::Write,
            Priority::Interactive,
            move |services, control| crate::writes::record_use(services, control, id, action),
        )
    }
}
