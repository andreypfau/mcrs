//! In-process scale-bot harness.
//!
//! Boots a minimal ECS world that exercises the bridge_outbound pipeline
//! with N synthetic "bot" entities instead of real TCP connections. Bots
//! inject outbound activity so the per-connection outbound queues run under
//! producer load. A configurable fraction of bots perform cross-dim
//! transfers (reassign their session's placement) halfway through the
//! run to exercise the session mutation + teardown path.
//!
//! The runner is tick-bounded (`run_profile_ticks`) so it is deterministic: a
//! fixed number of ticks covers every code path (injection, cross-dim
//! reassignment, queue routing, teardown) without spinning on the wall clock.
//!
//! Every injected packet carries the originating bot's real `PlayerSession`,
//! so `bridge_outbound` resolves it against the sessions and routes it to
//! that bot's `OutboundQueue`. The harness can then assert, with no
//! dependency on process-global metric atomics, that routing landed every
//! packet and that teardown leaves no session behind.

#![allow(dead_code)]

use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBlockUpdate;
use std::time::Instant;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::World;
use bevy_ecs::system::{IntoSystem, System};
use mcrs_minecraft_level::session::{
    Place, PlayerSession, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_server::world::bridge::bridge_outbound;
use mcrs_minecraft_server::world::bridge_queue::OutboundQueue;
use mcrs_minecraft_server::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_server::world::session::{HostAnchorRef, SessionBundle};

/// Per-run report: the functional invariants the bench asserts on, all local
/// to this run's `World`, alongside soft observational telemetry.
#[derive(Debug)]
pub struct ScaleReport {
    pub profile_name: String,
    pub dims: usize,
    pub bots_total: usize,
    pub snapshot_start: BridgeTelemetry,
    pub snapshot_end: BridgeTelemetry,
    /// entity count at T=0
    pub entity_count_start: u64,
    /// entity count at T=end
    pub entity_count_end: u64,
    /// monotone consumed count at T=0
    pub consumed_start: u64,
    /// monotone consumed count at T=end
    pub consumed_end: u64,
    /// wall-clock of the shortest tick observed (µs)
    pub tick_min_us: u64,
    /// wall-clock of the longest tick observed (µs)
    pub tick_max_us: u64,
    /// mean tick wall-clock (µs), total elapsed / tick_count
    pub tick_mean_us: u64,
    /// total ticks executed
    pub tick_count: u64,
    /// number of packets the harness wrote into the bus over the run
    pub packets_injected: u64,
    /// total packets resident across all per-bot `OutboundQueue`s at T=end,
    /// i.e. how many injected packets `bridge_outbound` actually routed
    pub total_queued: u64,
    /// number of session cross-dim reassignments performed
    pub cross_dim_transfers: u64,
    /// bot sessions still present after teardown (expect 0)
    pub sessions_remaining_after_teardown: u64,
}

impl ScaleReport {
    /// Entity-count delta: negative = cleaned up, zero = stable, positive = leak.
    pub fn entity_delta(&self) -> i64 {
        self.entity_count_end as i64 - self.entity_count_start as i64
    }
}

pub fn run_profile_ticks(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    ticks: u64,
) -> ScaleReport {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<BridgeTelemetry>();

    // Synthetic dimension entities — plain entity handles used as dim keys
    // in session placements. No dim sub-app is spawned; the harness exercises
    // the bridge_outbound queue-routing path only (no sub-app extract closure).
    let dim_entities: Vec<Entity> = (0..dims.max(1)).map(|_| world.spawn_empty().id()).collect();

    // One OutboundQueue entity per bot (no real socket or ServerSideConnection;
    // dispatch_encode requires ServerSideConnection to send bytes, so the
    // harness targets bridge_outbound queue-fill under bot load). Entity-count
    // delta across the run asserts the session teardown is clean.
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

    // Initialise systems once before the tick loop.
    let mut sys_outbound = IntoSystem::into_system(bridge_outbound);
    sys_outbound.initialize(&mut world);

    // T=0 snapshot.
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
        // Drives the one-shot cross-dim transfer at the halfway mark.
        let elapsed_frac = tick_count as f32 / ticks as f32;

        let tick_start = Instant::now();

        // Every 2 ticks, inject one BlockUpdate outbound packet per bot,
        // stamped with that bot's real session so bridge_outbound resolves it
        // and routes it to the bot's OutboundQueue. BlockUpdate is a MAPPED
        // variant so it exercises the real fill path.
        if tick_count.is_multiple_of(2) {
            for (player, _socket, session) in &bot_entities {
                world
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .write(OutboundPlayerPacket {
                        target: PacketTarget::SinglePlayer(*player),
                        priority: PacketPriority::Normal,
                        data: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                            block_pos: mcrs_minecraft_core::BlockPos::new(0, 64, 0),
                            block_state_id: BlockStateId(1),
                        }),
                        session: *session,
                        epoch: 0,
                    });
                packets_injected += 1;
            }
        }

        // Halfway through: reassign a fraction of bots to a different dim to
        // exercise the session cross-dim mutation path.
        if elapsed_frac >= 0.5 && !cross_dim_triggered && dim_entities.len() > 1 {
            cross_dim_triggered = true;
            let transfer_count = (bots_total as f32 * cross_dim_rate.clamp(0.0, 1.0)) as usize;
            for (player, _socket, _session) in bot_entities.iter().take(transfer_count) {
                if let Some(mut placement) = world.get_mut::<SessionPlacement>(*player)
                    && let Some(old_dim) = placement.place().dim()
                {
                    let idx = dim_entities.iter().position(|&d| d == old_dim).unwrap_or(0);
                    let new_dim = dim_entities[(idx + 1) % dim_entities.len()];
                    // Keeps the epoch: every injected packet is stamped with epoch 0.
                    *placement = SessionPlacement::new(
                        Place::Transferring {
                            from: old_dim,
                            to: new_dim,
                        },
                        placement.epoch(),
                    );
                    cross_dim_transfers += 1;
                }
            }
        }

        // Drain the outbound message bus into per-bot queues.
        let _ = sys_outbound.run((), &mut world);
        sys_outbound.apply_deferred(&mut world);

        let tick_elapsed = tick_start.elapsed().as_micros() as u64;
        tick_min_us = tick_min_us.min(tick_elapsed);
        tick_max_us = tick_max_us.max(tick_elapsed);
        tick_count += 1;
    }

    let total_elapsed = run_start.elapsed();
    let tick_mean_us = if tick_count > 0 {
        total_elapsed.as_micros() as u64 / tick_count
    } else {
        0
    };
    if tick_min_us == u64::MAX {
        tick_min_us = 0;
    }

    // Count packets that bridge_outbound actually routed into per-bot queues.
    let total_queued: u64 = bot_entities
        .iter()
        .map(|(_player, socket, _session)| {
            world
                .get::<OutboundQueue>(*socket)
                .map(|q| q.total_len() as u64)
                .unwrap_or(0)
        })
        .sum();

    // Tear down all bot sessions to validate the cleanup path.
    for (player, _socket, _session) in &bot_entities {
        world.entity_mut(*player).remove::<SessionBundle>();
    }
    let sessions_remaining_after_teardown: u64 = bot_entities
        .iter()
        .filter(|(player, _s, _session)| world.get::<Session>(*player).is_some())
        .count() as u64;

    // T=end snapshot.
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
