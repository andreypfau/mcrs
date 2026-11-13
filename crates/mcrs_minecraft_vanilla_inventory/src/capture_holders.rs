use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;

use crate::PlayerHolders;
use crate::close_inventory_menu::close_inventory_menu;
use crate::hand_launch::hand_launch;
use crate::player_stacks::CRAFT_GRID_SLOTS;
use crate::player_stacks::PlayerStacks;

/// The stacks of the holders of the player for a capture with `cause`, or `None` when it has no
/// inventory. A leave and a dimension change first close the inventory menu (INV-CLS-003); a
/// cause that this crate does not know captures as an autosave.
#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that a capture needs"
)]
pub(crate) fn capture_holders(
    In((player, cause)): In<(Entity, CaptureCause)>,
    mut commands: Commands,
    table: Res<ItemTable>,
    players: Query<(
        &Slots,
        Option<&Selected>,
        Option<&PlayerHolders>,
        Option<&Transform>,
        Option<&GameProfile>,
    )>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
) -> Option<PlayerStacks> {
    let (slots, selected, player_holders, transform, profile) = players.get(player).ok()?;
    let mut values = PlayerStacks::empty();
    values.selected = selected.map_or(0, |selected| selected.0);
    read_slots(&mut stacks, slots, &mut values.inventory);
    if let Some(player_holders) = player_holders {
        if let Ok(cursor) = holders.get(player_holders.cursor) {
            values.cursor = cursor.get(0).and_then(|stack| stacks.to_value(stack));
        }
        if let Ok(craft_grid) = holders.get(player_holders.craft_grid) {
            read_slots(&mut stacks, craft_grid, &mut values.craft_grid);
        }
    }
    let closes = matches!(cause, CaptureCause::Leave | CaptureCause::DimensionChange);
    if closes && values.has_carried() {
        let rests = close_inventory_menu(&mut values, &table);
        let player_holders = player_holders.copied();
        let transform = transform.copied().unwrap_or(Transform::IDENTITY);
        let source = profile.map_or(0, |profile| profile.id.as_u128());
        apply_close(&mut commands, player, player_holders, &values);
        drop_rests(&mut commands, rests, transform, source);
    }
    Some(values)
}

fn read_slots(stacks: &mut Stacks, slots: &Slots, values: &mut [Option<ItemStack>]) {
    for (value, stack) in values.iter_mut().zip(slots.iter()) {
        *value = stack.and_then(|stack| stacks.to_value(stack));
    }
}

/// Makes the holders of the player hold what the close gave: the player inventory with the
/// stacks that entered it, and the cursor, the craft grid and the craft result empty.
fn apply_close(
    commands: &mut Commands,
    player: Entity,
    holders: Option<PlayerHolders>,
    values: &PlayerStacks,
) {
    commands
        .entity(player)
        .load_inventory(values.inventory.clone());
    let Some(holders) = holders else {
        return;
    };
    commands.entity(holders.cursor).load_inventory(vec![None]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(vec![None; CRAFT_GRID_SLOTS]);
    commands
        .entity(holders.craft_result)
        .load_inventory(vec![None]);
}

/// Throws each rest from the player as one item entity (IEN-SPN-001, IEN-SPN-006).
fn drop_rests(commands: &mut Commands, rests: Vec<ItemStack>, transform: Transform, source: u128) {
    for rest in rests {
        commands
            .spawn_stack(rest)
            .launch(hand_launch(transform, source, Cause::MenuRest));
    }
}
