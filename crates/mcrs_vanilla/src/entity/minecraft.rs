use mcrs_core::{rl, StaticRegistry};

use super::EntityType;

pub static PRIMED_TNT: EntityType = EntityType::new(rl!("minecraft:tnt"), 132);
pub static PLAYER: EntityType = EntityType::new(rl!("minecraft:player"), 155);

pub fn register_all_entity_types(registry: &mut StaticRegistry<EntityType>) {
    registry.register(PRIMED_TNT.identifier, &PRIMED_TNT);
    registry.register(PLAYER.identifier, &PLAYER);
}
