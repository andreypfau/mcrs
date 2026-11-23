use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundEntityPositionSync, PositionPath,
};
use std::time::Instant;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageRegistry;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::World;
use bevy_ecs::system::{IntoSystem, System};
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::muxer::bridge::bridge_outbound;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Apart,
    Together,
}

#[derive(Debug)]
#[expect(
    dead_code,
    reason = "the bot_load bench, the only target that includes this module, asserts on the entity, queue and teardown counts and leaves the rest of the run's telemetry unread"
)]
pub struct ScaleReport {
    pub profile_name: String,
    pub dims: usize,
    pub bots_total: usize,
    pub snapshot_start: BridgeTelemetry,
    pub snapshot_end: BridgeTelemetry,
    pub entity_count_start: u64,
    pub entity_count_end: u64,
    pub consumed_start: u64,
    pub consumed_end: u64,
    pub tick_min_us: u64,
    pub tick_max_us: u64,
    pub tick_mean_us: u64,
    pub tick_count: u64,
    pub packets_injected: u64,
    pub total_queued: u64,
    pub cross_dim_transfers: u64,
    pub sessions_remaining_after_teardown: u64,
}

impl ScaleReport {
    pub fn entity_delta(&self) -> i64 {
        self.entity_count_end as i64 - self.entity_count_start as i64
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the harness builds its world, injects the bots' packets and tears the sessions down in one tick loop, so the run reads as the sequence it measures"
)]
pub fn run_profile_ticks(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    ticks: u64,
    layout: Layout,
) -> ScaleReport {
    let mut world = World::new();
    MessageRegistry::register_message::<OutboundPlayerPacket>(&mut world);
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<BridgeTelemetry>();
    world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();
    world.init_resource::<mcrs_minecraft_engine_world::session::SessionAnchors>();

    let dim_entities: Vec<Entity> = (0..dims.max(1)).map(|_| world.spawn_empty().id()).collect();

    let bot_entities: Vec<(Entity, Entity, PlayerSession)> = (0..bots_total)
        .map(|i| {
            let dim = dim_entities[i % dim_entities.len()];
            let socket = world.spawn(OutboundQueue::default()).id();
            let player = world.spawn_empty().id();
            let session = world.resource_mut::<PlayerSessionCounter>().next();
            world.entity_mut(player).insert(SessionBundle::placed(
                session,
                SessionPlacement::new(Place::InDim(dim), 0),
            ));
            world.entity_mut(socket).insert(HostAnchorRef(player));
            (player, socket, session)
        })
        .collect();

    let mut sys_outbound = IntoSystem::into_system(bridge_outbound);
    sys_outbound.initialize(&mut world);

    let snapshot_start = *world.resource::<BridgeTelemetry>();
    let consumed_start = snapshot_start.outbound_messages_consumed_total;
    let entity_count_start = world.entities().len() as u64;

    let run_start = Instant::now();

    let mut tick_count: u64 = 0;
    let mut tick_min_us = u64::MAX;
    let mut tick_max_us = 0u64;
    let mut packets_injected: u64 = 0;
    let mut cross_dim_transfers: u64 = 0;
    let mut cross_dim_triggered = false;

    loop {
        if tick_count >= ticks {
            break;
        }
        let elapsed_frac = tick_count as f32 / ticks as f32;

        let tick_start = Instant::now();

        if layout == Layout::Together {
            for (index, (_player, _socket, session)) in bot_entities.iter().enumerate() {
                let Some(dim) = dim_entities.get(index % dim_entities.len()) else {
                    continue;
                };
                world
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .write(OutboundPlayerPacket {
                        target: PacketTarget::AllInDim(*dim),
                        data: PacketPayload::EntityPosSync(ClientboundEntityPositionSync {
                            entity_id: VarInt(index as i32),
                            position: PositionPath::Linear(DVec3::new(
                                tick_count as f64 * 0.6,
                                64.0,
                                0.0,
                            )),
                            look: Look {
                                yaw: 0.0,
                                pitch: 0.0,
                            },
                            on_ground: true,
                        }),
                        session: *session,
                        epoch: 0,
                    });
                packets_injected += 1;
            }
        } else if tick_count.is_multiple_of(2) {
            for (_player, _socket, session) in &bot_entities {
                world
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .write(OutboundPlayerPacket {
                        target: PacketTarget::SinglePlayer(*session),
                        data: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                            block_pos: mcrs_voxel::BlockPos::new(0, 64, 0),
                            block_state_id: BlockStateNumber(1),
                        }),
                        session: *session,
                        epoch: 0,
                    });
                packets_injected += 1;
            }
        }

        if elapsed_frac >= 0.5 && !cross_dim_triggered && dim_entities.len() > 1 {
            cross_dim_triggered = true;
            let transfer_count = (bots_total as f32 * cross_dim_rate.clamp(0.0, 1.0)) as usize;
            for (player, _socket, _session) in bot_entities.iter().take(transfer_count) {
                if let Some(mut placement) = world.get_mut::<SessionPlacement>(*player)
                    && let Some(old_dim) = placement.place().dim()
                {
                    let idx = dim_entities.iter().position(|&d| d == old_dim).unwrap_or(0);
                    let new_dim = dim_entities[(idx + 1) % dim_entities.len()];
                    *placement = SessionPlacement::new(Place::InDim(new_dim), placement.epoch());
                    cross_dim_transfers += 1;
                }
            }
        }

        let _ = sys_outbound.run((), &mut world);
        sys_outbound.apply_deferred(&mut world);

        let tick_elapsed = tick_start.elapsed().as_micros() as u64;
        tick_min_us = tick_min_us.min(tick_elapsed);
        tick_max_us = tick_max_us.max(tick_elapsed);
        tick_count += 1;
    }

    let total_elapsed = run_start.elapsed();
    let tick_mean_us = (total_elapsed.as_micros() as u64)
        .checked_div(tick_count)
        .unwrap_or(0);
    if tick_min_us == u64::MAX {
        tick_min_us = 0;
    }

    let total_queued: u64 = bot_entities
        .iter()
        .map(|(_player, socket, _session)| {
            world
                .get::<OutboundQueue>(*socket)
                .map(|q| q.total_len() as u64)
                .unwrap_or(0)
        })
        .sum();

    for (player, _socket, _session) in &bot_entities {
        world.entity_mut(*player).remove::<SessionBundle>();
    }
    let sessions_remaining_after_teardown: u64 = bot_entities
        .iter()
        .filter(|(player, _s, _session)| world.get::<Session>(*player).is_some())
        .count() as u64;

    let snapshot_end = *world.resource::<BridgeTelemetry>();
    let consumed_end = snapshot_end.outbound_messages_consumed_total;
    let entity_count_end = world.entities().len() as u64;

    ScaleReport {
        profile_name: name.to_string(),
        dims,
        bots_total,
        snapshot_start,
        snapshot_end,
        entity_count_start,
        entity_count_end,
        consumed_start,
        consumed_end,
        tick_min_us,
        tick_max_us,
        tick_mean_us,
        tick_count,
        packets_injected,
        total_queued,
        cross_dim_transfers,
        sessions_remaining_after_teardown,
    }
}
