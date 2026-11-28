use crate::ClickSync;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateId(u16);

impl StateId {
    const MASK: u16 = 0x7FFF;

    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1) & Self::MASK)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub fn sync_after_click(self, packet_state_id: i32) -> ClickSync {
        if i32::from(self.0) == packet_state_id {
            ClickSync::CompareEntries
        } else {
            ClickSync::FullContent
        }
    }
}
