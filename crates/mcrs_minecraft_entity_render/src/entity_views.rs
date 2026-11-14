use bevy::prelude::*;

use crate::records::CULL_VIEWS_SHIFT;

/// The set of views that draw the entity, one bit for each view. An entity with no
/// `EntityViews` draws in the world view only. A change of the set is a change of the value, not
/// of the archetype.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityViews(pub u8);

impl EntityViews {
    /// The view of the camera of the window.
    pub const WORLD: Self = Self(1);
    /// The view of a figure that a host draws with its own projection, such as a GUI.
    pub const FIGURE: Self = Self(2);

    /// The set as the high byte of [`crate::CullRecord::flags`].
    #[must_use]
    pub const fn cull_flags(self) -> u16 {
        (self.0 as u16) << CULL_VIEWS_SHIFT
    }

    /// The set as [`crate::FrameUniform::view`].
    #[must_use]
    pub const fn frame_bit(self) -> u32 {
        self.0 as u32
    }
}
