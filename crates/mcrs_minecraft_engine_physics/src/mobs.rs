use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_protocol_game::entity::MobLooks;

use crate::mob_swing::MobSwing;

#[derive(Component, Clone, Debug, PartialEq)]
#[require(MobSwing)]
pub struct Mob {
    pub kind: &'static EntityTypeData,
    pub uuid: u128,
    pub looks: MobLooks,
    pub custom_name: Option<String>,
    pub tags: Vec<String>,
}
