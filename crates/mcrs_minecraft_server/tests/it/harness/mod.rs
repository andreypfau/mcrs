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
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_server::world::aoi::{PlayerTrackerPlugin, TrackedBy};
use mcrs_minecraft_server::world::bus::{InboundPlayerDespawn, OutboundPlayerPacket};
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::entity::player::column_view::ColumnView;

/// Build a host App with the AoI plugin, the outbound bus, and the
/// `FixedPreUpdate` / `FixedPostUpdate` schedules registered.
///
/// `InboundPlayerDespawn` is registered because `PlayerTrackerPlugin`
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

/// Run the AoI tick pair: `FixedPreUpdate` for the despawn drain, then
/// `FixedPostUpdate` for the observer mirror and the AoI system.
pub fn drive_aoi_tick(app: &mut App) {
    app.world_mut().run_schedule(FixedPreUpdate);
    app.world_mut().run_schedule(FixedPostUpdate);
}

/// The columns a client standing at `pos` holds once its view, at the default
/// view distance, has been sent.
pub fn columns_in_view(pos: DVec3) -> Vec<ColumnPos> {
    let centre = ColumnPos::from(pos);
    let radius = PlayerViewDistance::default().distance as i32;
    (-radius..=radius)
        .flat_map(|dx| {
            (-radius..=radius).map(move |dz| ColumnPos::new(centre.x + dx, centre.z + dz))
        })
        .collect()
}

/// Spawn a player entity holding every column of its view, as a player whose
/// view has finished loading does.
pub fn spawn_player_in_dim(app: &mut App, dim: Entity, pos: DVec3) -> Entity {
    app.world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ColumnView::holding(columns_in_view(pos)),
            TrackedBy::default(),
            InDimension(dim),
        ))
        .id()
}

/// Spawn a player entity that also carries `HostAnchor(host_anchor)`.
/// Used for tests that exercise the cross-world drain path, where the
/// drain system resolves host_anchor → in-dim Player via HostAnchor —
/// the same component production attaches in `consume_inbound_player_spawn`.
pub fn spawn_player_in_dim_with_host_anchor(
    app: &mut App,
    dim: Entity,
    pos: DVec3,
    host_anchor: Entity,
) -> Entity {
    app.world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ColumnView::holding(columns_in_view(pos)),
            TrackedBy::default(),
            InDimension(dim),
            HostAnchor(host_anchor),
        ))
        .id()
}

/// Run a single FixedPreUpdate schedule pass.
pub fn run_fixed_pre_update(app: &mut App) {
    app.world_mut().run_schedule(FixedPreUpdate);
}

/// Run a single FixedPostUpdate schedule pass.
pub fn run_fixed_post_update(app: &mut App) {
    app.world_mut().run_schedule(FixedPostUpdate);
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
