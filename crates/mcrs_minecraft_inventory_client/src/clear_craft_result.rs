use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn clear_craft_result(
    player_holders: &PlayerHolders,
    holders: &Query<&Slots>,
    commands: &mut Commands,
) {
    if let Some(result) =
        (holders.get(player_holders.craft_result).ok()).and_then(|slots| slots.get(0))
    {
        commands.entity(result).despawn();
    }
}
