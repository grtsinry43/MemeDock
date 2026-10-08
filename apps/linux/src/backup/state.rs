#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Creating,
    Reading,
    Preview,
    Restoring,
}
impl Phase {
    pub fn busy(self) -> bool {
        matches!(self, Self::Creating | Self::Reading | Self::Restoring)
    }
}
