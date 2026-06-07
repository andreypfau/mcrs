//! Per-dim system writing `ChunkSubscriptionSet` on each moved player
//! AND mirror-writing the same delta into each subscribed column's
//! `PlayerObservers`. The mirror happens inside the same loop iteration
//! as the subscription mutation — the AOI mirror invariant
//! (`aoi_mirror_invariant.rs`) hinges on that atomicity.

use std::sync::atomic::Ordering;

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{
    Added, Changed, Commands, Entity, Or, Query, ResMut, With, Without,
};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::PlayerViewDistance;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::storage::column::{Column, ColumnIndex};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use mcrs_protocol::chunk::LightData;

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
    mut commands: Commands,
) {
    probe.own_pov_ran = probe.own_pov_ran.saturating_add(1);

    for (player, transform, view_distance, in_dim, mut subscriptions) in players.iter_mut() {
        let Ok(column_index) = column_indices.get(in_dim.0) else {
            continue;
        };

        let centre = ColumnPos::from(transform.translation);
        let radius = view_distance.distance as i32;
        // Chebyshev (square) ball `max(|dx|, |dz|) <= radius` matches
        // `ChunkTrackingView::contains` and vanilla view-distance semantics.
        // A Manhattan (diamond) iterator would under-cover the corner
        // columns of the visible square.
        //
        // Only positions backed by a live ColumnIndex entry enter the
        // desired set: ChunkSubscriptionSet mirrors PlayerObservers
        // (asserted by aoi_mirror_invariant.rs), and recording a
        // subscription on a column that has no entity would silently
        // violate that invariant. Worldgen-edge positions re-enter the
        // set on a later tick once their column has been generated.
        let mut desired: FxHashSet<ColumnPos> = FxHashSet::default();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let pos = ColumnPos::new(centre.x + dx, centre.z + dz);
                if column_index.0.contains_key(&pos) {
                    desired.insert(pos);
                }
            }
        }

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
            let Some(slot) = column_index.0.get(pos) else {
                continue;
            };
            match observers.get_mut(slot.entity) {
                Ok(mut obs) => {
                    if !obs.0.contains(&player) {
                        obs.0.push(player);
                    }
                }
                Err(_) => {
                    // Column entity exists in ColumnIndex but PlayerObservers
                    // has not been seeded yet (the seeder runs in
                    // FixedPreUpdate; a column spawned in FixedUpdate first
                    // reaches us here in FixedPostUpdate without it).
                    // Commands::insert would overwrite the whole Component at
                    // flush time — if two players both hit this arm for the
                    // same column in the same tick, the second insert silently
                    // discards the first player. commands.queue runs a closure
                    // with &mut World at flush time; sequential closures for the
                    // same column each observe the world state left by the
                    // previous one, so push operations compose rather than
                    // clobber.
                    let player_id = player;
                    let column_entity = slot.entity;
                    commands.queue(move |world: &mut bevy_ecs::world::World| {
                        if let Some(mut obs) =
                            world.get_mut::<PlayerObservers>(column_entity)
                        {
                            if !obs.0.contains(&player_id) {
                                obs.0.push(player_id);
                            }
                        } else if let Ok(mut entity_mut) =
                            world.get_entity_mut(column_entity)
                        {
                            // get_entity_mut no-ops if the column was
                            // despawned between this system body and
                            // command flush (e.g., a ticket release
                            // raced ahead of FixedPostUpdate).
                            // entity_mut would panic in that case.
                            entity_mut.insert(PlayerObservers(SmallVec::from_slice(
                                &[player_id],
                            )));
                        }
                    });
                }
            }
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(player),
                priority: PacketPriority::Critical,
                // chunk_bytes and light_data will be populated by the per-dim chunk
                // producer; placeholder empty bytes until that system is wired.
                data: PacketPayload::ChunkLoad {
                    column: *pos,
                    chunk_bytes: Vec::new(),
                    light_data: LightData::default(),
                },
            });
            mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                .fetch_add(1, Ordering::Relaxed);
        }

        for pos in &removed {
            // A position appears in `removed` only if it was previously
            // recorded in `subscriptions` and is no longer in `desired`.
            // The added-loop above only inserts subscriptions for
            // positions backed by ColumnIndex, so a removal arm reaching
            // here without a slot means the column was despawned out
            // from under the subscription — the despawn already cleared
            // PlayerObservers via Component drop. Skip the wire-emit so
            // the client does not receive an unload for a chunk it never
            // saw a load for.
            let Some(slot) = column_index.0.get(pos) else {
                continue;
            };
            if let Ok(mut obs) = observers.get_mut(slot.entity) {
                obs.0.retain(|e| *e != player);
            }
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(player),
                priority: PacketPriority::Normal,
                data: PacketPayload::ChunkUnload { column: *pos },
            });
            mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                .fetch_add(1, Ordering::Relaxed);
        }

        subscriptions.0 = desired;
    }
}
