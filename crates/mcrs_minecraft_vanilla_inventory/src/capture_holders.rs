use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;

use crate::PlayerHolders;
use crate::carried_codec::has_carried;
use crate::close_inventory_menu::apply_close_moves;
use crate::close_inventory_menu::close_inventory_menu;
use crate::close_place::Place;
use crate::close_place::close_place;
use crate::from_holders::from_holders;
use crate::hand_launch::hand_launch;

#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that a capture needs"
)]
pub(crate) fn capture_holders(
    In((player, cause)): In<(Entity, CaptureCause)>,
    world: &World,
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
) -> Option<PlayerItems> {
    let (slots, selected, player_holders, transform, profile) = players.get(player).ok()?;
    let mut values = from_holders(
        slots,
        selected.map_or(0, |selected| selected.0),
        player_holders,
        &holders,
        &mut stacks,
    );
    let closes = matches!(cause, CaptureCause::Leave | CaptureCause::DimensionChange);
    if closes && has_carried(&values) {
        let place = |from, index| {
            player_holders.map_or(Place::OPEN, |holders| {
                close_place(world, player, *holders, from, index)
            })
        };
        let moves = close_inventory_menu(&values, &table, place);
        let rests = apply_close_moves(&mut values, &moves);
        let player_holders = player_holders.copied();
        let transform = transform.copied().unwrap_or(Transform::IDENTITY);
        let source = profile.map_or(0, |profile| profile.id.as_u128());
        apply_close(&mut commands, player, player_holders, &values);
        drop_rests(&mut commands, rests, transform, source);
    }
    Some(values)
}

fn apply_close(
    commands: &mut Commands,
    player: Entity,
    holders: Option<PlayerHolders>,
    values: &PlayerItems,
) {
    commands
        .entity(player)
        .load_inventory(values.slots.to_vec());
    let Some(holders) = holders else {
        return;
    };
    commands.entity(holders.cursor).load_inventory(vec![None]);
    commands
        .entity(holders.craft_grid)
        .load_inventory(vec![None; values.craft_grid.len()]);
    commands
        .entity(holders.craft_result)
        .load_inventory(vec![None]);
}

fn drop_rests(commands: &mut Commands, rests: Vec<ItemStack>, transform: Transform, source: u128) {
    for rest in rests {
        commands
            .spawn_stack(rest)
            .launch(hand_launch(transform, source, Cause::MenuRest));
    }
}
