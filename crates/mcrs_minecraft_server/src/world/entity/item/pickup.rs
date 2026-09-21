use crate::world::aoi::TrackedBy;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::item::{ITEM_HEIGHT, ITEM_WIDTH};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::item::click::{insert_stack, room_for};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_item::{DroppedItem, Held, ItemStack, Items, mutate};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_protocol::GameMode;
use smallvec::SmallVec;

const PLAYER_HALF_WIDTH: f64 = 0.3;
const PLAYER_HEIGHT: f64 = 1.8;
const REACH_XZ: f64 = 1.0;
const REACH_Y: f64 = 0.5;

/// The take packet goes out before the stack moves: the despawn that a full
/// pickup performs sends the remove packet synchronously, and the client only
/// animates a take for an entity it still has.
pub fn pickup_items(world: &mut World) {
    let mut players = world.query_filtered::<(Entity, &InDimension, &Transform, Option<&PlayerGameMode>), With<Player>>();
    let players: Vec<(Entity, Entity, DVec3, bool)> = players
        .iter(world)
        .filter(|(_, _, _, mode)| mode.is_none_or(|mode| mode.0 != GameMode::Spectator))
        .map(|(player, dim, transform, mode)| {
            let creative = mode.is_some_and(|mode| mode.0 == GameMode::Creative);
            (player, dim.0, transform.translation, creative)
        })
        .collect();
    if players.is_empty() {
        return;
    }
    let mut dropped = world.query::<(Entity, &InDimension, &Transform, &DroppedItem)>();
    let ready: Vec<(Entity, Entity, DVec3)> = dropped
        .iter(world)
        .filter(|(_, _, _, item)| item.pickup_delay == 0)
        .map(|(entity, dim, transform, _)| (entity, dim.0, transform.translation))
        .collect();
    if ready.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    for (player, player_dim, at, creative) in players {
        for &(item, item_dim, item_at) in &ready {
            if item_dim != player_dim || !touching(at, item_at) || world.get_entity(item).is_err() {
                continue;
            }
            let Some(count) = world.get::<ItemStack>(item).map(|stack| stack.count()) else {
                continue;
            };
            let room = room_for(world, player, item, &items);
            if room == 0 && !creative {
                continue;
            }
            let targets: SmallVec<[Entity; 8]> = std::iter::once(player)
                .chain(
                    world
                        .get::<TrackedBy>(item)
                        .into_iter()
                        .flat_map(|tracked| tracked.0.iter().copied()),
                )
                .filter_map(|viewer| world.get::<HostAnchor>(viewer).map(|anchor| anchor.0))
                .collect::<SmallVec<[Entity; 8]>>();
            let mut targets = targets;
            targets.sort_unstable();
            targets.dedup();
            world
                .resource_mut::<Messages<OutboundPlayerPacket>>()
                .write(OutboundPlayerPacket {
                    target: PacketTarget::PlayerSet(targets),
                    priority: PacketPriority::Normal,
                    data: PacketPayload::TakeItemEntity {
                        item_id: item.index_u32() as i32,
                        player_id: player.index_u32() as i32,
                        amount: i32::from(count),
                    },
                    session: PlayerSession(0),
                    epoch: 0,
                });
            if room == 0 {
                world.despawn(item);
                continue;
            }
            let taking = u8::try_from(room).map_or(count, |room| room.min(count));
            let Some(taken) = mutate::split(world, item, taking, &items) else {
                continue;
            };
            if let Err(error) = insert_stack(world, player, taken, &items) {
                tracing::debug!(%error, ?player, "a picked up stack could not be stored");
            }
            let unplaced =
                world.get::<ItemStack>(taken).is_some() && world.get::<Held>(taken).is_none();
            if unplaced {
                if world.get_entity(item).is_ok() {
                    mutate::merge_into(world, taken, item, u8::MAX);
                }
                if world.get::<ItemStack>(taken).is_some() {
                    tracing::warn!(?player, "a picked up remainder had nowhere to go");
                    world.despawn(taken);
                }
            }
        }
    }
}

fn touching(player: DVec3, item: DVec3) -> bool {
    (item.x - player.x).abs() < PLAYER_HALF_WIDTH + REACH_XZ + ITEM_WIDTH / 2.0
        && (item.z - player.z).abs() < PLAYER_HALF_WIDTH + REACH_XZ + ITEM_WIDTH / 2.0
        && item.y < player.y + PLAYER_HEIGHT + REACH_Y
        && item.y + ITEM_HEIGHT > player.y - REACH_Y
}
