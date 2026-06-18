//! Protocol-aware disconnect path.
//!
//! Single source of truth for player-going-away events. The
//! `on_player_disconnect` observer fires synchronously on every removal of
//! [`ServerSideConnection`]; it records the host-anchor into
//! `DisconnectedThisTick` and either processes the cleanup directly (if the
//! per-tick `DisconnectBudget` is available) or defers it to
//! `PendingDisconnectQueue`. A queue hard cap protects against unbounded
//! growth under mass-disconnect attack; overflow drops increment
//! `OverflowCounter` (and emit a warn log).
//!
//! The `filter_inflight_for_disconnect` system runs in `Update` after the
//! observer flush and drops in-flight bus messages addressed to a just-
//! disconnected host-anchor — `OutboundPlayerAttached` and any pending
//! lifecycle spawns/block events.
//!
//! `drain_pending_disconnects` runs in `First` to refill the budget and
//! process whatever was queued in earlier ticks.

use bevy_app::{App, First, Plugin, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Query, ResMut};
use mcrs_network::ServerSideConnection;
use smallvec::SmallVec;
use std::collections::VecDeque;
use tracing::warn;

use mcrs_engine::session::{PlayerSession, SessionRegistry};
use crate::world::bus::{OutboundPlayerAttached, OutboundPlayerDisconnect};
use crate::world::channel_types::{send_control_or_teardown, DimChannelsResource, ToDim};
use mcrs_engine::world::sub_app::DimDespawnQueue;
use crate::world::player_index::{HostAnchorRef, PlayerIndex, PlayerSessionRef};

/// Per-tick cleanup budget. The initial 32 caps work at 640 disconnects/sec
/// under a 20 TPS schedule, draining a 1000-player kick in ~1.5s without
/// monopolising a tick.
#[derive(Resource)]
pub struct DisconnectBudget {
    pub remaining: u32,
    pub max_per_tick: u32,
}

impl Default for DisconnectBudget {
    fn default() -> Self {
        Self {
            remaining: 32,
            max_per_tick: 32,
        }
    }
}

impl DisconnectBudget {
    pub fn consume(&mut self) -> bool {
        if self.remaining > 0 {
            self.remaining -= 1;
            true
        } else {
            false
        }
    }

    pub fn refill(&mut self) {
        self.remaining = self.max_per_tick;
    }
}

/// Hard cap on the deferred-cleanup queue. Overflow drops the entry and
/// increments `OverflowCounter`; the threshold prevents OOM-as-DoS while
/// still absorbing brief bursts above the per-tick budget.
pub const QUEUE_HARD_CAP: usize = 10_000;

#[derive(Resource, Default)]
pub struct PendingDisconnectQueue {
    pub entries: VecDeque<Entity>,
}

impl PendingDisconnectQueue {
    /// Push a host-anchor; returns `false` if the hard cap is reached and
    /// the entry was dropped.
    pub fn push_back(&mut self, host_anchor: Entity) -> bool {
        if self.entries.len() >= QUEUE_HARD_CAP {
            false
        } else {
            self.entries.push_back(host_anchor);
            true
        }
    }

    pub fn pop_front(&mut self) -> Option<Entity> {
        self.entries.pop_front()
    }
}

/// Set of host-anchors removed in the current tick. Populated by the
/// disconnect observer and drained by `filter_inflight_for_disconnect` at
/// the end of the same `Update` schedule.
#[derive(Resource, Default)]
pub struct DisconnectedThisTick {
    pub host_anchors: SmallVec<[Entity; 32]>,
}

/// Increment-only counter for queue-hard-cap drop events. Read by tests
/// (the only deterministic way to assert the drop happened without
/// taking on a `tracing_test`-style log-capture dependency) and also
/// exposed as a steady-state telemetry surface so an operator can scrape
/// the value alongside `AoiTickProbe`.
///
/// The observer emits a `warn!` log on the first drop of a fresh
/// `OverflowCounter` and then every `OVERFLOW_HEARTBEAT_INTERVAL` drops
/// thereafter so a sustained drop storm does not flood the log surface.
#[derive(Resource, Default)]
pub struct OverflowCounter(pub u32);

/// Heartbeat cadence for the overflow-drop warning. Tunable here so a
/// sustained storm produces at most one warning per `INTERVAL` drops
/// (plus the always-on first-drop signal). Picked to give roughly one
/// log line per few seconds at the per-tick budget ceiling.
pub const OVERFLOW_HEARTBEAT_INTERVAL: u32 = 256;

/// Observer over `On<Remove, ServerSideConnection>`. Fires synchronously
/// in the command-flush boundary, so `DisconnectedThisTick` is populated
/// before any later `Update` system runs in the same tick.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "disconnect::on_player_disconnect", skip_all)
)]
pub fn on_player_disconnect(
    trigger: On<Remove, ServerSideConnection>,
    connection_refs: Query<&HostAnchorRef>,
    mut player_index: ResMut<PlayerIndex>,
    mut session_registry: ResMut<SessionRegistry>,
    mut disconnect_budget: ResMut<DisconnectBudget>,
    mut pending_queue: ResMut<PendingDisconnectQueue>,
    mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
    mut overflow_counter: ResMut<OverflowCounter>,
    dim_channels: ResMut<DimChannelsResource>,
    mut despawn_queue: ResMut<DimDespawnQueue>,
    mut commands: Commands,
) {
    let connection_entity = trigger.event().entity;
    let Ok(host_anchor_ref) = connection_refs.get(connection_entity) else {
        return;
    };
    let host_anchor = host_anchor_ref.0;
    disconnected_this_tick.host_anchors.push(host_anchor);

    if disconnect_budget.consume() {
        process_disconnect(
            host_anchor,
            &mut player_index,
            &mut session_registry,
            &dim_channels,
            &mut despawn_queue,
            &mut commands,
        );
    } else if !pending_queue.push_back(host_anchor) {
        let before = overflow_counter.0;
        overflow_counter.0 = before.saturating_add(1);
        let after = overflow_counter.0;
        if before == 0 || after.is_multiple_of(OVERFLOW_HEARTBEAT_INTERVAL) {
            warn!(
                target: "disconnect",
                ?host_anchor,
                overflow_total = after,
                "PendingDisconnectQueue hard-cap exceeded; dropping disconnect"
            );
        }
    }
}

/// Run a single host-anchor's cleanup: resolve the session from
/// `SessionRegistry`, route an `InboundPlayerDespawn` into both `current_dim`
/// and `previous_dim` (if set — handles mid-transit disconnects), remove the
/// `SessionRegistry` entry, remove the `PlayerIndex` username entry, and
/// despawn the host-anchor entity.
///
/// Despawning into a dim that never saw the entity is harmless: the dest
/// sub-app ignores despawn messages for unknown host-anchors. The dual
/// emit is the chosen trade-off for sub-case-1 idempotency.
pub fn process_disconnect(
    host_anchor: Entity,
    player_index: &mut PlayerIndex,
    session_registry: &mut SessionRegistry,
    dim_channels: &DimChannelsResource,
    despawn_queue: &mut DimDespawnQueue,
    commands: &mut Commands,
) {
    let (session, current_dim, previous_dim, connection_entity) =
        match session_registry.get_by_anchor(&host_anchor) {
            Some((s, e)) => (*s, e.dim, e.previous_dim, e.connection_entity),
            None => return,
        };

    let _ = session;
    if let Some(chan) = dim_channels.get(current_dim) {
        send_control_or_teardown(
            &chan.control_sender,
            current_dim,
            ToDim::Despawn { host_anchor },
            despawn_queue,
        );
    }

    if let Some(prev) = previous_dim
        && prev != current_dim
    {
        if let Some(chan) = dim_channels.get(prev) {
            send_control_or_teardown(
                &chan.control_sender,
                prev,
                ToDim::Despawn { host_anchor },
                despawn_queue,
            );
        }
    }

    session_registry.remove(&session);

    if let Ok(mut socket_entity) = commands.get_entity(connection_entity) {
        socket_entity.try_remove::<crate::world::bridge_queue::OutboundQueue>();
    }

    if let Ok(mut anchor_entity) = commands.get_entity(host_anchor) {
        anchor_entity.despawn();
    }
}

/// `First`-schedule system: refill the budget, then drain the queue up to
/// the budget. Each popped entry consumes a budget slot before running
/// `process_disconnect`.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "disconnect::drain_pending_disconnects", skip_all)
)]
pub fn drain_pending_disconnects(
    mut disconnect_budget: ResMut<DisconnectBudget>,
    mut pending_queue: ResMut<PendingDisconnectQueue>,
    mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
    mut player_index: ResMut<PlayerIndex>,
    mut session_registry: ResMut<SessionRegistry>,
    dim_channels: ResMut<DimChannelsResource>,
    mut despawn_queue: ResMut<DimDespawnQueue>,
    mut commands: Commands,
) {
    disconnect_budget.refill();
    while disconnect_budget.remaining > 0 {
        let Some(host_anchor) = pending_queue.pop_front() else {
            break;
        };
        disconnect_budget.remaining -= 1;
        disconnected_this_tick.host_anchors.push(host_anchor);
        process_disconnect(
            host_anchor,
            &mut player_index,
            &mut session_registry,
            &dim_channels,
            &mut despawn_queue,
            &mut commands,
        );
    }
}

/// `Update`-schedule system: drop in-flight bus messages whose
/// `host_anchor` was just disconnected this tick. Clears the
/// `DisconnectedThisTick` set at the end so the next tick starts fresh.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "disconnect::filter_inflight_for_disconnect", skip_all)
)]
pub fn filter_inflight_for_disconnect(
    mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
    mut attached_msgs: ResMut<Messages<OutboundPlayerAttached>>,
    mut disconnect_msgs: ResMut<Messages<OutboundPlayerDisconnect>>,
) {
    if disconnected_this_tick.host_anchors.is_empty() {
        return;
    }
    let disconnected: rustc_hash::FxHashSet<Entity> =
        disconnected_this_tick.host_anchors.iter().copied().collect();

    let kept_attached: Vec<OutboundPlayerAttached> = attached_msgs
        .drain()
        .filter(|msg| !disconnected.contains(&msg.host_anchor))
        .collect();
    for msg in kept_attached {
        attached_msgs.write(msg);
    }

    let kept_disconnects: Vec<OutboundPlayerDisconnect> = disconnect_msgs
        .drain()
        .filter(|msg| !disconnected.contains(&msg.host_anchor))
        .collect();
    for msg in kept_disconnects {
        disconnect_msgs.write(msg);
    }

    disconnected_this_tick.host_anchors.clear();
}

/// Wires the disconnect protocol: four host resources, the observer, and
/// the two systems.
pub struct DisconnectProtocolPlugin;

impl Plugin for DisconnectProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisconnectBudget>();
        app.init_resource::<PendingDisconnectQueue>();
        app.init_resource::<DisconnectedThisTick>();
        app.init_resource::<OverflowCounter>();
        app.init_resource::<DimDespawnQueue>();
        app.add_observer(on_player_disconnect);
        app.add_systems(First, drain_pending_disconnects);
        app.add_systems(
            Update,
            filter_inflight_for_disconnect
                .after(crate::world::bridge::bridge_inbound_to_channel),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_budget_default_is_32() {
        let b = DisconnectBudget::default();
        assert_eq!(b.remaining, 32);
        assert_eq!(b.max_per_tick, 32);
    }

    #[test]
    fn disconnect_budget_consume_decrements_until_zero() {
        let mut b = DisconnectBudget::default();
        for _ in 0..32 {
            assert!(b.consume());
        }
        assert!(!b.consume());
        assert_eq!(b.remaining, 0);
    }

    #[test]
    fn disconnect_budget_refill_resets_to_max() {
        let mut b = DisconnectBudget {
            remaining: 0,
            max_per_tick: 32,
        };
        b.refill();
        assert_eq!(b.remaining, 32);
    }

    #[test]
    fn pending_disconnect_queue_hard_cap_returns_false() {
        let mut q = PendingDisconnectQueue::default();
        let e = Entity::from_raw_u32(1).expect("nonzero");
        for _ in 0..QUEUE_HARD_CAP {
            assert!(q.push_back(e));
        }
        assert!(!q.push_back(e), "push past hard cap should return false");
        assert_eq!(q.entries.len(), QUEUE_HARD_CAP);
    }

    #[test]
    fn overflow_counter_default_is_zero() {
        assert_eq!(OverflowCounter::default().0, 0);
    }
}
