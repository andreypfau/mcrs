use std::collections::BTreeMap;

use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_replication::MobLooks;

/// The cold state of each mob row of the world, by its id.
#[derive(Resource, Debug, Default)]
pub struct Mobs(pub BTreeMap<EntityId, Mob>);

/// What a mob keeps outside its row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mob {
    pub kind: MobType,
    pub uuid: u128,
    pub looks: MobLooks,
}
