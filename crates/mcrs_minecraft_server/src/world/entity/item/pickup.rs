use crate::world::aoi::TrackedBy;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::item::{ITEM_HEIGHT, ITEM_WIDTH};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_inventory::{
    MenuSnapshot, Planner, Source, StackView, Transaction, player_menu_layout,
};
use mcrs_minecraft_item::{DroppedItem, Items};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundTakeItemEntity;
use rustc_hash::FxHashMap;

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
    // One snapshot per player for the tick, so two pickups cannot book the
    // same cell twice.
    let mut snapshots: FxHashMap<Entity, MenuSnapshot> = FxHashMap::default();
    for (player, player_dim, at, creative) in players {
        let mut ops = Vec::new();
        for &(item, item_dim, item_at) in &ready {
            if item_dim != player_dim || !touching(at, item_at) {
                continue;
            }
            let Some(view) = StackView::of(world, item, &items) else {
                continue;
            };
            let snapshot = snapshots.entry(player).or_insert_with(|| {
                MenuSnapshot::new(world, &items, player, player_menu_layout(player))
            });
            let mut planner = Planner::new(snapshot);
            let room = planner.room_for(&view);
            if room == 0 && !creative {
                continue;
            }
            let mut targets = TrackedBy::anchors(world, item);
            targets.extend(world.get::<HostAnchor>(player).map(|anchor| anchor.0));
            targets.sort_unstable();
            targets.dedup();
            world
                .resource_mut::<Messages<OutboundPlayerPacket>>()
                .write(OutboundPlayerPacket {
                    target: PacketTarget::PlayerSet(targets),
                    priority: PacketPriority::Normal,
                    data: PacketPayload::TakeItemEntity(ClientboundTakeItemEntity {
                        item_id: VarInt(item.index_u32() as i32),
                        player_id: VarInt(player.index_u32() as i32),
                        amount: VarInt(i32::from(view.count)),
                    }),
                    session: PlayerSession(0),
                    epoch: 0,
                });
            if room == 0 {
                world.despawn(item);
                continue;
            }
            planner.snapshot.add_item(item, view);
            planner.insert_stack(Source::Item(item));
            planner.snapshot.take(Source::Item(item));
            ops.extend(planner.ops);
        }
        if !ops.is_empty() {
            Transaction(ops).apply(world);
        }
    }
}

fn touching(player: DVec3, item: DVec3) -> bool {
    (item.x - player.x).abs() < PLAYER_HALF_WIDTH + REACH_XZ + ITEM_WIDTH / 2.0
        && (item.z - player.z).abs() < PLAYER_HALF_WIDTH + REACH_XZ + ITEM_WIDTH / 2.0
        && item.y < player.y + PLAYER_HEIGHT + REACH_Y
        && item.y + ITEM_HEIGHT > player.y - REACH_Y
}
