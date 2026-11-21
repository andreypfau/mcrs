use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use rustc_hash::FxHashMap;

#[derive(Resource, Debug, Default)]
pub struct Mobs(pub FxHashMap<EntityId, Mob>);

#[derive(Resource, Debug, Default)]
pub struct MobUuids(pub FxHashMap<u128, EntityId>);

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Mob {
    pub kind: &'static EntityTypeData,
    pub uuid: u128,
    pub looks: MobLooks,
    pub custom_name: Option<String>,
    pub tags: Vec<String>,
}
