use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::entity::player::Player;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::full_sends::FullSends;
use crate::player_stacks::PlayerStacks;

/// Gives the player its four holders with `stacks`. The holders that the player had keep their
/// entities and lose their stacks. The player then waits for the send of its full state.
pub(crate) fn restore_holders(
    In((player, stacks)): In<(Entity, PlayerStacks)>,
    mut commands: Commands,
    mut sends: ResMut<FullSends>,
    players: Query<Option<&PlayerHolders>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(&mut commands, player, holders);
    commands
        .entity(player)
        .load_inventory(stacks.inventory)
        .insert(Selected(stacks.selected))
        .insert_if_new(InventoryMenuSync::default());
    commands
        .entity(holders.cursor)
        .load_inventory(vec![stacks.cursor]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(stacks.craft_grid.into());
    commands
        .entity(holders.craft_result)
        .load_inventory(vec![None]);
    send_full_state(&mut sends, player);
}

/// Puts the cursor and the craft grid of `stacks` into the holders of the player.
pub(crate) fn restore_carried(
    In((player, stacks)): In<(Entity, PlayerStacks)>,
    mut commands: Commands,
    mut sends: ResMut<FullSends>,
    players: Query<Option<&PlayerHolders>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(&mut commands, player, holders);
    commands
        .entity(holders.cursor)
        .load_inventory(vec![stacks.cursor]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(stacks.craft_grid.into());
    send_full_state(&mut sends, player);
}

/// Gives a new player four empty holders and the selected slot 0 (INV-SLT-002). A player file or
/// a handover that the engine applies after the spawn fills the same holders.
pub(crate) fn hold_new_player(
    add: On<Add<Player>>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
) {
    let player = add.entity;
    if let Ok(None) = players.get(player) {
        commands.run_system_cached_with(restore_holders, (player, PlayerStacks::empty()));
    }
}

fn holders_of(
    commands: &mut Commands,
    player: Entity,
    holders: Option<&PlayerHolders>,
) -> PlayerHolders {
    if let Some(holders) = holders {
        return *holders;
    }
    let holders = PlayerHolders {
        cursor: commands.spawn(ChildOf(player)).id(),
        craft_grid: commands.spawn(ChildOf(player)).id(),
        craft_result: commands.spawn(ChildOf(player)).id(),
    };
    commands.entity(player).insert(holders);
    holders
}

fn send_full_state(sends: &mut FullSends, player: Entity) {
    if !sends.0.contains(&player) {
        sends.0.push(player);
    }
}
