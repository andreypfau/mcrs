use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_player::Holder;

use crate::PlayerHolders;
use crate::PlayerItemsRestored;
use crate::off_hand_gate::OffHandGate;
use crate::player_policy::player_policy;

/// Gives the player its four holders with `stacks`. The holders that the player had keep their
/// entities and lose their stacks.
pub(crate) fn restore_holders(
    In((player, stacks)): In<(Entity, PlayerItems)>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
    off_hand: Option<Res<OffHandGate>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(&mut commands, player, holders, off_hand.as_deref());
    commands
        .entity(player)
        .load_inventory(stacks.slots.into())
        .insert(Selected(stacks.selected));
    commands
        .entity(holders.cursor)
        .load_inventory(vec![stacks.cursor]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(stacks.craft_grid.into());
    commands
        .entity(holders.craft_result)
        .load_inventory(vec![None]);
    commands.trigger(PlayerItemsRestored { entity: player });
}

/// Puts the cursor and the craft grid of `stacks` into the holders of the player.
pub(crate) fn restore_carried(
    In((player, stacks)): In<(Entity, PlayerItems)>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
    off_hand: Option<Res<OffHandGate>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(&mut commands, player, holders, off_hand.as_deref());
    commands
        .entity(holders.cursor)
        .load_inventory(vec![stacks.cursor]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(stacks.craft_grid.into());
    commands.trigger(PlayerItemsRestored { entity: player });
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
        commands.run_system_cached_with(restore_holders, (player, PlayerItems::default()));
    }
}

/// The holders of the player. A player without them gets new ones, and each of its four holders
/// gets its [`SlotPolicy`](mcrs_minecraft_engine_item::SlotPolicy).
fn holders_of(
    commands: &mut Commands,
    player: Entity,
    holders: Option<&PlayerHolders>,
    off_hand: Option<&OffHandGate>,
) -> PlayerHolders {
    if let Some(holders) = holders {
        return *holders;
    }
    let off_hand = off_hand.map_or(Gate::Permits, |gate| gate.0);
    let mut holder = |holder: Holder| {
        let policy = player_policy(holder, off_hand);
        commands.spawn((ChildOf(player), policy)).id()
    };
    let holders = PlayerHolders {
        cursor: holder(Holder::Cursor),
        craft_grid: holder(Holder::CraftGrid),
        craft_result: holder(Holder::CraftResult),
    };
    let policy = player_policy(Holder::Inventory, off_hand);
    commands.entity(player).insert((holders, policy));
    holders
}
