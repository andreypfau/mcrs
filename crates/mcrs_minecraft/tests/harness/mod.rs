//! Minimal AoI test harness.
//!
//! Each test builds a single-`App` host wired with `PlayerTrackerPlugin`
//! and explicitly runs `FixedPreUpdate` -> `FixedPostUpdate` per tick.
//! Going through `MinimalPlugins` + `Time<Fixed>` accumulation works but
//! adds wall-clock-coupled flakiness; the direct schedule run makes the
//! tests deterministic.

#![allow(dead_code)]

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::PlayerViewDistance;
use mcrs_engine::world::dimension::InDimension;
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, PlayerTrackerPlugin, TrackedBy};
use mcrs_minecraft::world::bus::{InboundPlayerDespawn, OutboundPlayerPacket};

/// Build a host App with the AoI plugin, the outbound bus, and the
/// `FixedPreUpdate` / `FixedPostUpdate` schedules registered.
///
/// `InboundPlayerDespawn` is registered because `PlayerTrackerPlugin` now
/// installs `drain_inbound_player_despawn` which reads `MessageReader<InboundPlayerDespawn>`.
pub fn make_aoi_app() -> App {
    let mut app = App::new();
    app.add_schedule(Schedule::new(FixedPreUpdate));
    app.add_schedule(Schedule::new(FixedPostUpdate));
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_plugins(PlayerTrackerPlugin);
    app
}

/// Run the AoI tick pair (`FixedPreUpdate` for the PlayerObservers
/// seeder, then `FixedPostUpdate` for the AoI systems).
pub fn drive_aoi_tick(app: &mut App) {
    app.world_mut().run_schedule(FixedPreUpdate);
    app.world_mut().run_schedule(FixedPostUpdate);
}

/// Spawn a player entity carrying the AoI bundle pieces required by
/// `update_own_pov` and `update_tracked_by`. The Transform and
/// PlayerViewDistance defaults are kept small (vd=12) to keep the
/// outward iteration cost low.
pub fn spawn_player_in_dim(app: &mut App, dim: Entity, pos: DVec3) -> Entity {
    app.world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ChunkSubscriptionSet::default(),
            TrackedBy::default(),
            InDimension(dim),
        ))
        .id()
}

/// Drain the host's outbound bus, returning every packet emitted since
/// the last call. The cursor pattern matches the production extract
/// closure but we read here for assertions instead.
pub fn drain_outbound(app: &mut App) -> Vec<OutboundPlayerPacket> {
    let mut buf = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>();
    buf.drain().collect()
}
