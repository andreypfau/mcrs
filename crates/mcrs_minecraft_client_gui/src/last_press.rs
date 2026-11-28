use core::time::Duration;

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LastPress {
    pub(crate) button: MouseButton,
    pub(crate) target: Option<u16>,
    pub(crate) menu: Entity,
    pub(crate) at: Duration,
}
