//! In-process scale-bot harness.
//!
//! Boots a minimal ECS world that exercises the bridge_outbound pipeline
//! with N synthetic "bot" entities instead of real TCP connections. Bots
//! inject outbound activity at a configurable rate so the outbound queue
//! and drop policy run under producer load. A configurable fraction of bots
//! perform cross-dim transfers (reassign current_dim) halfway through the
//! run to exercise the PlayerIndex cleanup path.
//!
//! Duration is driven by the `TR07_DURATION_SECS` env var (default 10 s)
//! so the smoke variant fits within the CI feedback budget. The full
//! 5-minute baseline run is invoked manually with `TR07_DURATION_SECS=300`.
//!
//! The emitted counter (`BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL`) is
//! incremented at every harness write so the emitted-vs-consumed gap is
//! observable as a bus-saturation dimension distinct from drop/kick counters.

#![allow(dead_code)]

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::World;
use bevy_ecs::system::{IntoSystem, System};
use mcrs_minecraft::world::bridge::bridge_outbound;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use mcrs_network::metrics::{
    snapshot, BridgeTelemetrySnapshot, BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL,
};
use mcrs_protocol::BlockStateId;
use smallvec::SmallVec;

/// Per-run observational report. Contains T=0 and T=end telemetry snapshots,
/// entity-count delta, per-tick timing statistics, and emitted/consumed totals.
#[derive(Debug)]
pub struct ScaleReport {
    pub profile_name: String,
    pub dims: usize,
    pub bots_total: usize,
    pub duration_secs: u64,
    pub snapshot_start: BridgeTelemetrySnapshot,
    pub snapshot_end: BridgeTelemetrySnapshot,
    /// entity count at T=0
    pub entity_count_start: u64,
    /// entity count at T=end
    pub entity_count_end: u64,
    /// monotone emitted count at T=0
    pub emitted_start: u64,
    /// monotone emitted count at T=end
    pub emitted_end: u64,
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
}

impl ScaleReport {
    /// Entity-count delta: negative = cleaned up, zero = stable, positive = leak.
    pub fn entity_delta(&self) -> i64 {
        self.entity_count_end as i64 - self.entity_count_start as i64
    }

    /// Bus-saturation gap: emitted minus consumed over the run. A positive
    /// gap means the consumer could not drain as fast as the producer wrote;
    /// it is a soft observational dimension, NOT a pass/fail gate.
    pub fn saturation_gap(&self) -> i64 {
        let emitted_delta = (self.emitted_end - self.emitted_start) as i64;
        let consumed_delta = (self.consumed_end - self.consumed_start) as i64;
        emitted_delta - consumed_delta
    }
}

/// Duration to run profiles — reads `TR07_DURATION_SECS` env var (default 10).
pub fn profile_duration_secs() -> u64 {
    std::env::var("TR07_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// Run a scale profile for `duration_secs` of wall clock.
///
/// `name` – profile label for the baseline JSON filename.
/// `dims` – number of synthetic dimension entities.
/// `bots_total` – total number of synthetic bot connection entities.
/// `cross_dim_rate` – fraction of bots (0.0–1.0) reassigned to a different
///   dim entity halfway through the run to exercise the PlayerIndex cleanup
///   invariant.
/// `duration_secs` – wall-clock run length in seconds.
pub fn run_profile(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    duration_secs: u64,
) -> ScaleReport {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();

    // Synthetic dimension entities — plain entity handles used as dim keys
    // in PlayerIndex. No dim sub-app is spawned; the harness exercises the
    // bridge_outbound queue-routing path only (no sub-app extract closure).
    let dim_entities: Vec<Entity> = (0..dims.max(1))
        .map(|_| world.spawn_empty().id())
        .collect();

    // One OutboundQueue entity per bot (no real socket or ServerSideConnection;
    // dispatch_encode requires ServerSideConnection to send bytes, so the
    // harness targets bridge_outbound queue-fill under bot load). Entity-count
    // delta across the run asserts the PlayerIndex teardown is clean.
    let bot_entities: Vec<(Entity, Entity)> = (0..bots_total)
        .map(|i| {
            let dim = dim_entities[i % dim_entities.len()];
            let socket = world.spawn(OutboundQueue::default()).id();
            let player = world.spawn_empty().id();
            world.resource_mut::<PlayerIndex>().insert(
                player,
                PlayerLocation {
                    socket,
                    current_dim: dim,
                    previous_dim: None,
                    in_dim_entity: Some(socket),
                    inbound_pending: SmallVec::new(),
                },
            );
            (player, socket)
        })
        .collect();

    // Initialise systems once before the tick loop.
    let mut sys_outbound = IntoSystem::into_system(bridge_outbound);
    sys_outbound.initialize(&mut world);

    // T=0 snapshot.
    let snapshot_start = snapshot();
    let emitted_start = BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.load(Ordering::Relaxed);
    let consumed_start = snapshot_start.outbound_messages_consumed_total;
    let entity_count_start = world.entities().len() as u64;

    let run_start = Instant::now();
    let deadline = run_start + Duration::from_secs(duration_secs);

    let mut tick_count: u64 = 0;
    let mut tick_min_us = u64::MAX;
    let mut tick_max_us = 0u64;
    let mut cross_dim_triggered = false;

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }

        let tick_start = Instant::now();

        // Every 2 ticks, inject one BlockUpdate outbound packet per bot.
        // BlockUpdate is a MAPPED variant (fully encoded by dispatch_encode)
        // so it exercises the real outbound queue fill path even without a
        // socket. The emitted counter is incremented here — not inside a
        // production sub-app system — so the harness-generated load is
        // visible in the emitted-vs-consumed saturation measurement.
        if tick_count % 2 == 0 {
            for (player, _socket) in &bot_entities {
                world
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .write(OutboundPlayerPacket {
                        target: PacketTarget::SinglePlayer(*player),
                        priority: PacketPriority::Normal,
                        data: PacketPayload::BlockUpdate {
                            position: mcrs_engine::geometry::BlockPos::new(0, 64, 0),
                            new_state: BlockStateId(1),
                        },
                    });
                BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Halfway through: reassign a fraction of bots to a different dim to
        // exercise the PlayerIndex cross-dim path. In_dim_entity is set to
        // None so bridge_outbound will drop these bots until they are re-attached.
        let elapsed_frac =
            now.duration_since(run_start).as_secs_f32() / duration_secs as f32;
        if elapsed_frac >= 0.5 && !cross_dim_triggered && dim_entities.len() > 1 {
            cross_dim_triggered = true;
            let transfer_count = (bots_total as f32 * cross_dim_rate.clamp(0.0, 1.0)) as usize;
            for (player, _socket) in bot_entities.iter().take(transfer_count) {
                if let Some(loc) = world.resource_mut::<PlayerIndex>().get_mut(player) {
                    let old_dim = loc.current_dim;
                    let idx = dim_entities
                        .iter()
                        .position(|&d| d == old_dim)
                        .unwrap_or(0);
                    let new_dim = dim_entities[(idx + 1) % dim_entities.len()];
                    loc.current_dim = new_dim;
                    loc.previous_dim = Some(old_dim);
                    loc.in_dim_entity = None;
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

    // Tear down all bot player_index entries to validate the cleanup path.
    for (player, _socket) in &bot_entities {
        world.resource_mut::<PlayerIndex>().remove(player);
    }

    // T=end snapshot.
    let snapshot_end = snapshot();
    let emitted_end = BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.load(Ordering::Relaxed);
    let consumed_end = snapshot_end.outbound_messages_consumed_total;
    let entity_count_end = world.entities().len() as u64;

    ScaleReport {
        profile_name: name.to_string(),
        dims,
        bots_total,
        duration_secs,
        snapshot_start,
        snapshot_end,
        entity_count_start,
        entity_count_end,
        emitted_start,
        emitted_end,
        consumed_start,
        consumed_end,
        tick_min_us,
        tick_max_us,
        tick_mean_us,
        tick_count,
    }
}

/// Serialize `report` to a JSON file at `path` for local baseline review.
pub fn write_baseline_json(report: &ScaleReport, path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;

    let json = serde_json::json!({
        "profile": report.profile_name,
        "dims": report.dims,
        "bots_total": report.bots_total,
        "duration_secs": report.duration_secs,
        "tick_count": report.tick_count,
        "tick_min_us": report.tick_min_us,
        "tick_max_us": report.tick_max_us,
        "tick_mean_us": report.tick_mean_us,
        "entity_count_start": report.entity_count_start,
        "entity_count_end": report.entity_count_end,
        "entity_delta": report.entity_delta(),
        "emitted_start": report.emitted_start,
        "emitted_end": report.emitted_end,
        "consumed_start": report.consumed_start,
        "consumed_end": report.consumed_end,
        "saturation_gap": report.saturation_gap(),
        "drop_normal_delta": report.snapshot_end.drop_normal_total
            - report.snapshot_start.drop_normal_total,
        "drop_low_delta": report.snapshot_end.drop_low_total
            - report.snapshot_start.drop_low_total,
        "kick_overflow_delta": report.snapshot_end.kick_overflow_total
            - report.snapshot_start.kick_overflow_total,
        "kick_flood_delta": report.snapshot_end.kick_flood_total
            - report.snapshot_start.kick_flood_total,
    });

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(serde_json::to_string_pretty(&json).unwrap().as_bytes())?;
    Ok(())
}
