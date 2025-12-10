use crate::world::movement;
use bevy_ecs::bundle::Bundle;
use crate::world::chunk_observer::VisibleEntities;

pub mod ability;
pub mod attribute;

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub visible_entities: VisibleEntities,
    pub abilities: ability::PlayerAbilitiesBundle,
    pub attributes: attribute::PlayerAttributesBundle,
}
