use crate::ClickSync;

/// The state id of a menu: 0 when the menu starts to exist, and one more, modulo 32768, before
/// each packet that sends the content or one slot of the menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateId(u16);

impl StateId {
    const MASK: u16 = 0x7FFF;

    /// The state id of a new menu.
    pub const INITIAL: Self = Self(0);

    /// The state id that the next content or slot packet carries.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1) & Self::MASK)
    }

    /// The value on the wire.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// What the server sends after a click whose packet carries `packet_state_id`.
    #[must_use]
    pub fn sync_after_click(self, packet_state_id: i32) -> ClickSync {
        if i32::from(self.0) == packet_state_id {
            ClickSync::CompareEntries
        } else {
            ClickSync::FullContent
        }
    }
}
