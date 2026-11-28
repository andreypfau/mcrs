use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::from_holders::from_holders;

pub(crate) fn capture_holders(world: &World, player: Entity) -> Option<PlayerItems> {
    let slots = world.get::<Slots>(player)?;
    Some(from_holders(
        slots,
        world
            .get::<Selected>(player)
            .map_or(0, |selected| selected.0),
        world.get::<PlayerHolders>(player),
        |holder| world.get::<Slots>(holder),
        |stack| Stacks::value_in(world, stack),
    ))
}
