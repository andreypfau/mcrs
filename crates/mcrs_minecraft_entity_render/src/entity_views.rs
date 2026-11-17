use bevy::prelude::*;

use crate::records::CULL_VIEWS_SHIFT;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityViews(pub u8);

impl EntityViews {
    pub const WORLD: Self = Self(1);
    pub const FIGURE: Self = Self(2);

    #[must_use]
    pub const fn cull_flags(self) -> u16 {
        (self.0 as u16) << CULL_VIEWS_SHIFT
    }

    #[must_use]
    pub const fn frame_bit(self) -> u32 {
        self.0 as u32
    }
}
