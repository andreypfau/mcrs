pub mod nibble;
pub mod storage;

use bevy_ecs::prelude::Component;

use crate::storage::LightStorage;

#[derive(Component, Clone, Debug, Default)]
pub struct BlockLight(pub LightStorage);

#[derive(Component, Clone, Debug, Default)]
pub struct SkyLight(pub LightStorage);
