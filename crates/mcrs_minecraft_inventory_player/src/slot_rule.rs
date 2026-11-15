use crate::SlotFilter;

/// The slot limit and the filters of one slot of a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotRule {
    pub limit: u8,
    pub filter: SlotFilter,
}
