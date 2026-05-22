//! Per-dim system writing `ChunkSubscriptionSet` on each moved player
//! AND mirror-writing the same delta into each subscribed column's
//! `PlayerObservers`. The mirror happens inside the same loop iteration
//! as the subscription mutation — the AOI mirror invariant
//! (`aoi_mirror_invariant.rs`) hinges on that atomicity.

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{
    Added, Changed, Entity, Or, Query, Res, ResMut, With, Without,
};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::PlayerViewDistance;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::math::outward::OutwardIterator2D;
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::{Column, ColumnIndex};
use rustc_hash::FxHashSet;

use crate::world::aoi::components::ChunkSubscriptionSet;
use crate::world::aoi::probe::AoiTickProbe;
use crate::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(
        name = "aoi::update_own_pov",
        skip_all,
        fields(moved_players = tracing::field::Empty)
    )
)]
#[allow(clippy::type_complexity)]
pub fn update_own_pov(
    mut probe: ResMut<AoiTickProbe>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &PlayerViewDistance,
            &InDimension,
            &mut ChunkSubscriptionSet,
        ),
        (
            With<Player>,
            Or<(Changed<Transform>, Added<ChunkSubscriptionSet>)>,
        ),
    >,
    mut observers: Query<&mut PlayerObservers, (With<Column>, Without<Player>)>,
    column_indices: Query<&ColumnIndex>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    probe.own_pov_ran = probe.own_pov_ran.saturating_add(1);

    for (player, transform, view_distance, in_dim, mut subscriptions) in players.iter_mut() {
        let Ok(column_index) = column_indices.get(in_dim.0) else {
            continue;
        };

        let centre = ColumnPos::from(transform.translation);
        let radius = view_distance.distance as i32;
        let desired: FxHashSet<ColumnPos> = OutwardIterator2D::new(centre.x, centre.z, radius)
            .map(|(x, z)| ColumnPos::new(x, z))
            .collect();

        let added: Vec<ColumnPos> = desired
            .iter()
            .copied()
            .filter(|pos| !subscriptions.0.contains(pos))
            .collect();
        let removed: Vec<ColumnPos> = subscriptions
            .0
            .iter()
            .copied()
            .filter(|pos| !desired.contains(pos))
            .collect();

        for pos in &added {
            if let Some(slot) = column_index.0.get(pos) {
                if let Ok(mut obs) = observers.get_mut(slot.entity) {
                    if !obs.0.contains(&player) {
                        obs.0.push(player);
                    }
                }
            }
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(player),
                priority: PacketPriority::Critical,
                data: PacketPayload::ChunkLoad { column: *pos },
            });
        }

        for pos in &removed {
            if let Some(slot) = column_index.0.get(pos) {
                if let Ok(mut obs) = observers.get_mut(slot.entity) {
                    obs.0.retain(|e| *e != player);
                }
            }
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(player),
                priority: PacketPriority::Normal,
                data: PacketPayload::ChunkUnload { column: *pos },
            });
        }

        subscriptions.0 = desired;
    }
}
