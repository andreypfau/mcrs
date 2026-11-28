use alloc::vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::Holder;

use crate::PlayerHolders;
use crate::armour_gates::ArmourGates;
use crate::player_policy::player_policy;

pub fn restore_holders(
    In((player, stacks)): In<(Entity, PlayerItems)>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
    armour: Option<Res<ArmourGates>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(commands.reborrow(), player, holders, armour.as_deref());
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
}

pub fn restore_carried(
    In((player, stacks)): In<(Entity, PlayerItems)>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
    armour: Option<Res<ArmourGates>>,
) {
    let Ok(holders) = players.get(player) else {
        return;
    };
    let holders = holders_of(commands.reborrow(), player, holders, armour.as_deref());
    commands
        .entity(holders.cursor)
        .load_inventory(vec![stacks.cursor]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(stacks.craft_grid.into());
}

pub(crate) fn hold_new_player(
    add: On<Add<Player>>,
    mut commands: Commands,
    players: Query<Option<&PlayerHolders>>,
    armour: Option<Res<ArmourGates>>,
) {
    let player = add.entity;
    if let Ok(None) = players.get(player) {
        holders_of(commands.reborrow(), player, None, armour.as_deref());
        commands.run_system_cached_with(restore_holders, (player, PlayerItems::default()));
    }
}

fn holders_of(
    mut commands: Commands<'_, '_>,
    player: Entity,
    holders: Option<&PlayerHolders>,
    armour: Option<&ArmourGates>,
) -> PlayerHolders {
    if let Some(holders) = holders {
        return *holders;
    }
    let armour = armour.copied().unwrap_or(ArmourGates::REFUSE);
    let mut holder = |holder: Holder| {
        let policy = player_policy(holder, armour);
        commands.spawn((ChildOf(player), policy)).id()
    };
    let holders = PlayerHolders {
        cursor: holder(Holder::Cursor),
        craft_grid: holder(Holder::CraftGrid),
        craft_result: holder(Holder::CraftResult),
    };
    let policy = player_policy(Holder::Inventory, armour);
    commands.entity(player).insert((holders, policy));
    if let Some(parts) = MenuParts::new([player, holders.craft_grid, holders.craft_result]) {
        commands
            .entity(player)
            .open_menu(MenuLayout::INVENTORY, parts, MenuButtons::default());
    }
    holders
}
