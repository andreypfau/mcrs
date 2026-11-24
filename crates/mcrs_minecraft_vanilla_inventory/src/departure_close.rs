use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::check_departure;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::is_own_dimension;
use mcrs_minecraft_engine::player_order::DepartureRequests;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;

use crate::carried_codec::has_carried;
use crate::close_inventory_menu::apply_close_moves;
use crate::close_inventory_menu::close_inventory_menu;
use crate::close_place::Place;
use crate::close_place::close_place;
use crate::from_holders::from_holders;
use crate::hand_launch::hand_launch;
use crate::launch_drops::eye_height_of;

type DepartingPlayer = (
    &'static Slots,
    Option<&'static Selected>,
    Option<&'static PlayerHolders>,
    Option<&'static Transform>,
    Option<&'static Pose>,
    Option<&'static AttributeInstances>,
    Option<&'static GameProfile>,
);

enum Departing {
    Leave(Entity),
    Written(ChangeDimensionRequest),
    Ordered(Entity),
}

#[derive(SystemParam)]
pub(crate) struct DepartureClose<'w, 's> {
    world: &'w World,
    commands: Commands<'w, 's>,
    table: Res<'w, ItemTable>,
    players: Query<'w, 's, DepartingPlayer, With<Player>>,
    holders: Query<'w, 's, &'static Slots>,
    stacks: Stacks<'w, 's>,
}

pub(crate) fn close_at_leave(
    mut leaving: MessageReader<InboundPlayerDespawn>,
    index: Option<Res<DimPlayerIndex>>,
    mut commands: Commands,
) {
    for left in leaving.read() {
        if let Some(&player) = index.as_ref().and_then(|index| index.0.get(&left.session)) {
            commands.run_system_cached_with(close_departing, Departing::Leave(player));
        }
    }
}

pub(crate) fn close_at_dimension_change(
    mut written: MessageReader<ChangeDimensionRequest>,
    ordered: Option<Res<DepartureRequests>>,
    mut commands: Commands,
) {
    for request in written.read() {
        commands.run_system_cached_with(close_departing, Departing::Written(request.clone()));
    }
    for player in ordered.iter().flat_map(|ordered| ordered.players()) {
        commands.run_system_cached_with(close_departing, Departing::Ordered(player));
    }
}

pub(crate) fn departures_requested(requests: Option<Res<DepartureRequests>>) -> bool {
    requests.is_some_and(|requests| requests.players().next().is_some())
}

fn close_departing(In(departing): In<Departing>, mut close: DepartureClose) {
    let world = close.world;
    let departs = |player| {
        world
            .get_entity(player)
            .is_ok_and(|player| check_departure(player).is_ok())
    };
    let player = match departing {
        Departing::Leave(player) => player,
        Departing::Written(request) => {
            if is_own_dimension(world, &request.to) || !departs(request.player) {
                return;
            }
            request.player
        }
        Departing::Ordered(player) => {
            if !departs(player) {
                return;
            }
            player
        }
    };
    close.close(player);
}

impl DepartureClose<'_, '_> {
    fn close(&mut self, player: Entity) {
        let Ok((slots, selected, player_holders, transform, pose, attributes, profile)) =
            self.players.get(player)
        else {
            return;
        };
        let mut values = from_holders(
            slots,
            selected.map_or(0, |selected| selected.0),
            player_holders,
            |holder| self.holders.get(holder).ok(),
            |stack| self.stacks.to_value(stack),
        );
        if has_carried(&values) {
            let world = self.world;
            let place = |from, index| {
                player_holders.map_or(Place::OPEN, |holders| {
                    close_place(world, player, *holders, from, index)
                })
            };
            let moves = close_inventory_menu(&values, &self.table, place);
            let rests = apply_close_moves(&mut values, &moves);
            let player_holders = player_holders.copied();
            let transform = transform.copied().unwrap_or(Transform::IDENTITY);
            let source = profile.map_or(0, |profile| profile.id.as_u128());
            apply_close(&mut self.commands, player, player_holders, &values);
            let eye_height = eye_height_of(pose, attributes);
            drop_rests(&mut self.commands, rests, transform, eye_height, source);
        }
    }
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

fn drop_rests(
    commands: &mut Commands,
    rests: Vec<ItemStack>,
    transform: Transform,
    eye_height: f32,
    source: u128,
) {
    for rest in rests {
        commands.spawn_stack(rest).launch(hand_launch(
            transform,
            eye_height,
            source,
            Cause::MenuRest,
        ));
    }
}
