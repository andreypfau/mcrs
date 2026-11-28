use bevy_ecs::resource::Resource;

use crate::bus::InboundPlayerSpawn;

#[derive(Resource, Debug, Default)]
pub struct HeldJoins(pub Vec<InboundPlayerSpawn>);
