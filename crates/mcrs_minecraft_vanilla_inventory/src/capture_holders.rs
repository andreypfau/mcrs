use bevy_ecs::entity::Entity;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::from_holders::from_holders;

pub(crate) fn capture_holders(
    In((player, _)): In<(Entity, CaptureCause)>,
    players: Query<(&Slots, Option<&Selected>, Option<&PlayerHolders>)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
) -> Option<PlayerItems> {
    let (slots, selected, player_holders) = players.get(player).ok()?;
    Some(from_holders(
        slots,
        selected.map_or(0, |selected| selected.0),
        player_holders,
        &holders,
        &mut stacks,
    ))
}
