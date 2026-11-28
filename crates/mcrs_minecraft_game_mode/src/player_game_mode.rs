use derive_more::{Deref, DerefMut};

use crate::GameMode;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Deref, DerefMut, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct PlayerGameMode(pub GameMode);

impl Default for PlayerGameMode {
    fn default() -> Self {
        Self(GameMode::Survival)
    }
}
