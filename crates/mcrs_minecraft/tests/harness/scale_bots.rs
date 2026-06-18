//! In-process scale-bot harness.
//!
//! Boots a minimal ECS world that exercises the bridge_outbound pipeline
//! with N synthetic "bot" entities instead of real TCP connections. Bots
//! inject outbound activity so the per-connection outbound queues run under
//! producer load. A configurable fraction of bots perform cross-dim
//! transfers (reassign their `SessionRegistry` entry) halfway through the
//! run to exercise the registry mutation + teardown path.
//!
//! The smoke runner is tick-bounded (`run_profile_ticks`) so it is
//! deterministic and finishes in milliseconds: a fixed number of ticks
//! covers every code path (injection, cross-dim reassignment, queue routing,
//! teardown) without spinning on the wall clock. The opt-in long baseline
//! runner (`run_profile`) is wall-clock-bounded and used only by the
//! `#[ignore]` observational variants invoked manually.
//!
//! Every injected packet carries the originating bot's real `PlayerSession`,
//! so `bridge_outbound` resolves it against the registry and routes it to
//! that bot's `OutboundQueue`. The harness can then assert, with no
//! dependency on process-global metric atomics, that routing landed every
//! packet and that teardown leaves the registry empty.

#![allow(dead_code)]

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::World;
use bevy_ecs::system::{IntoSystem, System};
use mcrs_engine::session::PlayerSession;
use mcrs_minecraft::world::bridge::bridge_outbound;
use mcrs_minecraft::world::bridge_queue::OutboundQueue;
use mcrs_minecraft::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_network::metrics::{
    snapshot, BridgeTelemetrySnapshot, BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL,
};
use mcrs_protocol::BlockStateId;

/// Per-run report. Carries the functional invariants the smoke tests assert
/// on (all local to this run's `World`, race-free under parallel test
/// execution) alongside the soft observational telemetry consumed by the
/// long-baseline JSON writer.
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
    /// number of packets the harness wrote into the bus over the run
    pub packets_injected: u64,
    /// total packets resident across all per-bot `OutboundQueue`s at T=end,
    /// i.e. how many injected packets `bridge_outbound` actually routed
    pub total_queued: u64,
    /// number of `SessionRegistry` cross-dim reassignments performed
    pub cross_dim_transfers: u64,
    /// bot sessions still present in the registry after teardown (expect 0)
    pub sessions_remaining_after_teardown: u64,
}

impl ScaleReport {
    /// Entity-count delta: negative = cleaned up, zero = stable, positive = leak.
    pub fn entity_delta(&self) -> i64 {
        self.entity_count_end as i64 - self.entity_count_start as i64
    }

    /// Bus-saturation gap: emitted minus consumed over the run. Derived from
    /// process-global metric atomics, so it is contaminated by any other test
    /// running concurrently — a SOFT observational dimension only, never a
    /// pass/fail gate. Use `total_queued` for race-free routing assertions.
    pub fn saturation_gap(&self) -> i64 {
        let emitted_delta = (self.emitted_end - self.emitted_start) as i64;
        let consumed_delta = (self.consumed_end - self.consumed_start) as i64;
        emitted_delta - consumed_delta
    }
}

/// How long a profile runs.
enum RunLength {
    /// Deterministic: execute exactly this many ticks.
    Ticks(u64),
    /// Wall-clock: run until this much time elapses (opt-in baselines only).
    Duration(Duration),
}

/// Duration to run the long baseline profiles — reads `TR07_DURATION_SECS`
/// env var (default 10).
pub fn profile_duration_secs() -> u64 {
    std::env::var("TR07_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// Run a scale profile for a fixed number of ticks. Deterministic and fast —
/// this is the smoke runner used by the always-on tests.
pub fn run_profile_ticks(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    ticks: u64,
) -> ScaleReport {
    run_profile_bounded(name, dims, bots_total, cross_dim_rate, RunLength::Ticks(ticks))
}

/// Run a scale profile for `duration_secs` of wall clock. Used only by the
/// opt-in `#[ignore]` baseline variants; not deterministic.
pub fn run_profile(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    duration_secs: u64,
) -> ScaleReport {
    run_profile_bounded(
        name,
        dims,
        bots_total,
        cross_dim_rate,
        RunLength::Duration(Duration::from_secs(duration_secs)),
    )
}

fn run_profile_bounded(
    name: &str,
    dims: usize,
    bots_total: usize,
    cross_dim_rate: f32,
    length: RunLength,
) -> ScaleReport {
    let mut world = World::new();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    world.init_resource::<PlayerIndex>();
    world.init_resource::<SessionRegistry>();
    world.init_resource::<PlayerSessionCounter>();

    // Synthetic dimension entities — plain entity handles used as dim keys
    // in SessionRegistry. No dim sub-app is spawned; the harness exercises
    // the bridge_outbound queue-routing path only (no sub-app extract closure).
    let dim_entities: Vec<Entity> = (0..dims.max(1))
        .map(|_| world.spawn_empty().id())
        .collect();

    // One OutboundQueue entity per bot (no real socket or ServerSideConnection;
    // dispatch_encode requires ServerSideConnection to send bytes, so the
    // harness targets bridge_outbound queue-fill under bot load). Entity-count
    // delta across the run asserts the SessionRegistry teardown is clean.
    let bot_entities: Vec<(Entity, Entity, PlayerSession)> = (0..bots_total)
        .map(|i| {
            let dim = dim_entities[i % dim_entities.len()];
            let socket = world.spawn(OutboundQueue::default()).id();
            let player = world.spawn_empty().id();
            let session = world.resource_mut::<PlayerSessionCounter>().next();
            world.resource_mut::<SessionRegistry>().insert(
                session,
                SessionEntry {
                    connection_entity: socket,
                    host_anchor: player,
                    dim,
                    previous_dim: None,
                    in_dim_entity: Some(socket),
                    epoch: 0,
                },
            );
            (player, socket, session)
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
    let deadline = match &length {
        RunLength::Ticks(_) => run_start,
        RunLength::Duration(d) => run_start + *d,
    };

    let mut tick_count: u64 = 0;
    let mut tick_min_us = u64::MAX;
    let mut tick_max_us = 0u64;
    let mut packets_injected: u64 = 0;
    let mut cross_dim_transfers: u64 = 0;
    let mut cross_dim_triggered = false;

    loop {
        // Fraction of the run completed, in [0, 1). Drives the one-shot
        // cross-dim transfer at the halfway mark for both bound kinds.
        let elapsed_frac = match &length {
            RunLength::Ticks(n) => {
                if tick_count >= *n {
                    break;
                }
                tick_count as f32 / *n as f32
            }
            RunLength::Duration(_) => {
                if Instant::now() >= deadline {
                    break;
                }
                run_start.elapsed().as_secs_f32()
                    / deadline.duration_since(run_start).as_secs_f32()
            }
        };

        let tick_start = Instant::now();

        // Every 2 ticks, inject one BlockUpdate outbound packet per bot,
        // stamped with that bot's real session so bridge_outbound resolves it
        // and routes it to the bot's OutboundQueue. BlockUpdate is a MAPPED
        // variant so it exercises the real fill path. The emitted counter is
        // bumped here so harness-generated load shows up in the soft
        // saturation telemetry.
        if tick_count % 2 == 0 {
            for (player, _socket, session) in &bot_entities {
                world
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .write(OutboundPlayerPacket {
                        target: PacketTarget::SinglePlayer(*player),
                        priority: PacketPriority::Normal,
                        data: PacketPayload::BlockUpdate {
                            position: mcrs_engine::geometry::BlockPos::new(0, 64, 0),
                            new_state: BlockStateId(1),
                        },
                        session: *session,
                        epoch: 0,
                    });
                BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.fetch_add(1, Ordering::Relaxed);
                packets_injected += 1;
            }
        }

        // Halfway through: reassign a fraction of bots to a different dim to
        // exercise the SessionRegistry cross-dim mutation path.
        if elapsed_frac >= 0.5 && !cross_dim_triggered && dim_entities.len() > 1 {
            cross_dim_triggered = true;
            let transfer_count = (bots_total as f32 * cross_dim_rate.clamp(0.0, 1.0)) as usize;
            for (_player, _socket, session) in bot_entities.iter().take(transfer_count) {
                if let Some(entry) = world.resource_mut::<SessionRegistry>().get_mut(session) {
                    let old_dim = entry.dim;
                    let idx = dim_entities
                        .iter()
                        .position(|&d| d == old_dim)
                        .unwrap_or(0);
                    let new_dim = dim_entities[(idx + 1) % dim_entities.len()];
                    entry.dim = new_dim;
                    entry.previous_dim = Some(old_dim);
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

    // Tear down all bot session registry entries to validate the cleanup path.
    for (_player, _socket, session) in &bot_entities {
        world.resource_mut::<SessionRegistry>().remove(session);
    }
    let sessions_remaining_after_teardown: u64 = {
        let registry = world.resource::<SessionRegistry>();
        bot_entities
            .iter()
            .filter(|(_p, _s, session)| registry.contains(session))
            .count() as u64
    };

    // T=end snapshot.
    let snapshot_end = snapshot();
    let emitted_end = BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.load(Ordering::Relaxed);
    let consumed_end = snapshot_end.outbound_messages_consumed_total;
    let entity_count_end = world.entities().len() as u64;

    ScaleReport {
        profile_name: name.to_string(),
        dims,
        bots_total,
        duration_secs: match &length {
            RunLength::Ticks(_) => 0,
            RunLength::Duration(d) => d.as_secs(),
        },
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
        packets_injected,
        total_queued,
        cross_dim_transfers,
        sessions_remaining_after_teardown,
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
        "packets_injected": report.packets_injected,
        "total_queued": report.total_queued,
        "cross_dim_transfers": report.cross_dim_transfers,
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
