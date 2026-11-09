use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use rustc_hash::FxHashMap;

/// The cold state of each mob row of the world, by its id.
#[derive(Resource, Debug, Default)]
pub struct Mobs(pub FxHashMap<EntityId, Mob>);

/// The id of each mob row of the world, by its UUID.
#[derive(Resource, Debug, Default)]
pub struct MobUuids(pub FxHashMap<u128, EntityId>);

/// What a mob keeps outside its row.
#[derive(Clone, Debug, PartialEq)]
pub struct Mob {
    pub kind: &'static EntityTypeData,
    pub uuid: u128,
    pub looks: MobLooks,
    pub custom_name: Option<String>,
    /// The scoreboard tags, which a selector tests.
    pub tags: Vec<String>,
}
