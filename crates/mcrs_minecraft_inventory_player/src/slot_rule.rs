use crate::SlotFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotRule {
    pub limit: u8,
    pub filter: SlotFilter,
}
