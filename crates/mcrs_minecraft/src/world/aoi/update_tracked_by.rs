//! Per-dim system deriving each moved player's `TrackedBy` cache from
//! the `PlayerObservers` sets of neighbouring columns inside a
//! tracking-radius window, then filtered by precise per-entity distance.
//! Emits `PlayerEnteredView` / `PlayerLeftView` delta packets via the
//! outbound bus.

use std::sync::atomic::Ordering;

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Entity, Query, ResMut, With, Without};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::{Column, ColumnIndex};
use smallvec::SmallVec;
use mcrs_protocol::uuid::Uuid;

use crate::login::GameProfile;
use crate::world::aoi::components::TrackedBy;
use crate::world::aoi::probe::AoiTickProbe;
use crate::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use crate::world::entity::MinecraftEntityType;

/// Chunk-column radius for player-to-player tracking. ~5 chunks ≈ 80
/// blocks; matches vanilla's mob/player track radius before
/// view-distance kicks in.
pub const TRACKING_RADIUS_CHUNKS: i32 = 5;

/// Precise distance-squared filter, in block units. Squared once so the
/// hot loop can skip the `sqrt` and only compare squared magnitudes.
pub const TRACKING_RADIUS_BLOCKS_SQ: f64 = 80.0 * 80.0;

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(
        name = "aoi::update_tracked_by",
        skip_all,
        fields(moved_players = tracing::field::Empty)
    )
)]
#[allow(clippy::type_complexity)]
pub fn update_tracked_by(
    mut probe: ResMut<AoiTickProbe>,
    mut moved_players: Query<
        (Entity, &Transform, &InDimension, &mut TrackedBy),
        (With<Player>, Changed<Transform>),
    >,
    all_players: Query<(Entity, &Transform, Option<&GameProfile>), With<Player>>,
    chunk_observers: Query<&PlayerObservers, (With<Column>, Without<Player>)>,
    column_indices: Query<&ColumnIndex>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    probe.tracked_by_ran = probe.tracked_by_ran.saturating_add(1);

    for (player, transform, in_dim, mut tracked_by) in moved_players.iter_mut() {
        let Ok(column_index) = column_indices.get(in_dim.0) else {
            continue;
        };
        let centre_chunk = ColumnPos::from(transform.translation);

        let mut new_observers: SmallVec<[Entity; 32]> = SmallVec::new();
        // Chebyshev (square) sweep to match the player view-distance
        // shape; a Manhattan diamond would leave corner columns invisible
        // and a player standing in a corner column would not be tracked.
        for dx in -TRACKING_RADIUS_CHUNKS..=TRACKING_RADIUS_CHUNKS {
            for dz in -TRACKING_RADIUS_CHUNKS..=TRACKING_RADIUS_CHUNKS {
                let pos = ColumnPos::new(centre_chunk.x + dx, centre_chunk.z + dz);
                let Some(slot) = column_index.0.get(&pos) else {
                    continue;
                };
                let Ok(observers) = chunk_observers.get(slot.entity) else {
                    continue;
                };
                for &other_entity in observers.0.iter() {
                    if other_entity == player {
                        continue;
                    }
                    if new_observers.contains(&other_entity) {
                        continue;
                    }
                    let Ok((_, other_xf, _)) = all_players.get(other_entity) else {
                        continue;
                    };
                    if transform
                        .translation
                        .distance_squared(other_xf.translation)
                        > TRACKING_RADIUS_BLOCKS_SQ
                    {
                        continue;
                    }
                    new_observers.push(other_entity);
                }
            }
        }

        for &new_entity in &new_observers {
            if !tracked_by.0.contains(&new_entity) {
                // Resolve the entered player's wire fields from the query.
                // UUID from GameProfile; position from Transform. The per-dim
                // producer will supply authoritative data when fully wired.
                let (uuid, pos) = all_players
                    .get(player)
                    .map(|(_, xf, profile)| {
                        let uuid = profile.map(|p| p.id).unwrap_or(Uuid::nil());
                        (uuid, xf.translation)
                    })
                    .unwrap_or((Uuid::nil(), transform.translation));
                packet_writer.write(OutboundPlayerPacket {
                    target: PacketTarget::SinglePlayer(new_entity),
                    priority: PacketPriority::Normal,
                    data: PacketPayload::PlayerEnteredView {
                        entity_id: player.index_u32() as i32,
                        uuid,
                        kind: MinecraftEntityType::Player as i32,
                        position: pos,
                        yaw: transform.rotation.y,
                        pitch: transform.rotation.x,
                    },
                });
                mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        for &old_entity in tracked_by.0.iter() {
            if !new_observers.contains(&old_entity) {
                let mut ids: SmallVec<[i32; 4]> = SmallVec::new();
                ids.push(player.index_u32() as i32);
                packet_writer.write(OutboundPlayerPacket {
                    target: PacketTarget::SinglePlayer(old_entity),
                    priority: PacketPriority::Normal,
                    data: PacketPayload::PlayerLeftView { entity_ids: ids },
                });
                mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        tracked_by.0 = new_observers;
    }
}
