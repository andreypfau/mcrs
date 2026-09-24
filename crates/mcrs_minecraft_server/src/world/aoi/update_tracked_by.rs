//! Per-dim system deriving each moved player's `TrackedBy` cache from
//! the `PlayerObservers` sets of neighbouring columns inside a
//! tracking-radius window, then filtered by precise per-entity distance.
//! Emits `PlayerEnteredView` / `PlayerLeftView` delta packets via the
//! outbound bus.

use crate::world::bus::to;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Entity, Query, With, Without};
use bevy_math::DVec3;
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex};
use mcrs_minecraft_protocol::ByteAngle;
use mcrs_minecraft_protocol::LpVec3;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundRemoveEntities;
use mcrs_minecraft_protocol::uuid::Uuid;
use smallvec::SmallVec;

use crate::login::GameProfile;
use crate::world::aoi::components::TrackedBy;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_world::entity::minecraft::PLAYER;

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
    mut moved_players: Query<
        (Entity, &Transform, &InDimension, &mut TrackedBy),
        (With<Player>, Changed<Transform>),
    >,
    all_players: Query<(Entity, &Transform, Option<&GameProfile>), With<Player>>,
    chunk_observers: Query<&PlayerObservers, (With<Column>, Without<Player>)>,
    column_indices: Query<&ColumnIndex>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
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
                    if transform.translation.distance_squared(other_xf.translation)
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
                packet_writer.write(to(
                    new_entity,
                    PacketPayload::PlayerEnteredView(ClientboundAddEntity {
                        id: VarInt(player.index_u32() as i32),
                        uuid,
                        kind: VarInt(PLAYER.protocol_id as i32),
                        pos,
                        movement: LpVec3(DVec3::ZERO),
                        yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
                        pitch: ByteAngle::from_degrees(transform.rotation.pitch()),
                        head_yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
                        data: VarInt(0),
                    }),
                ));
            }
        }
        for &old_entity in tracked_by.0.iter() {
            if !new_observers.contains(&old_entity) {
                packet_writer.write(to(
                    old_entity,
                    PacketPayload::PlayerLeftView(ClientboundRemoveEntities {
                        entity_ids: vec![VarInt(player.index_u32() as i32)],
                    }),
                ));
            }
        }
        tracked_by.0 = new_observers;
    }
}
