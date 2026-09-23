use std::sync::Arc;

use crate::world::light_codec::{LightCodecParams, build_delta_light_data};
use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_core::{ColumnPos, SectionPos};
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::block_update::BlockPlaced;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_level::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::prelude::LightWorkQueue;
use mcrs_minecraft_light::prelude::{
    Edit, LightBounds, LightPlugin, LightSet, PendingEdits, Priority, SectionRelit,
};

use mcrs_minecraft_worldgen_generator::heightmap::SurfaceHeightmap;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;

/// A dimension's side of the light engine: what it hands the light world before intake, and
/// what it sends once the light world has published.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimLightSet {
    Feed,
    Emit,
}

pub struct DimLightPlugin {
    pub registry: Arc<LightRegistry>,
    pub bounds: LightBounds,
    pub sky: bool,
}

impl Plugin for DimLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LightPlugin {
            registry: Arc::clone(&self.registry),
            bounds: self.bounds,
            sky: self.sky,
        })
        .configure_sets(
            Last,
            (
                DimLightSet::Feed.before(LightSet::Intake),
                DimLightSet::Emit.after(LightSet::Publish),
            ),
        )
        // Sections land in `FixedLast`, so `Last` is the first schedule of the
        // same tick that can see them.
        .add_systems(
            Last,
            (
                (
                    feed_light_edits,
                    reprioritize_light_work,
                    // After the block edits it bounds, and after the maps have taken
                    // this tick's edits: a bound must never describe blocks the
                    // light world has not been handed.
                    feed_column_surfaces.after(crate::world::heightmap::update_column_heightmaps),
                )
                    .chain()
                    .in_set(DimLightSet::Feed),
                emit_light_updates.in_set(DimLightSet::Emit),
            ),
        );
    }
}

/// Re-scores the waiting light work against where the players stand now.
///
/// A column is not sent until the light around it has settled, so work scored when it was
/// raised carries the distance from wherever the player was then. One flight across the world
/// and the queue is sorted by a position nobody occupies, which leaves the column under the
/// player waiting behind thousands raised earlier and further away.
fn reprioritize_light_work(
    mut pending: ResMut<PendingEdits>,
    mut queue: ResMut<LightWorkQueue>,
    players: Query<&Transform, With<Player>>,
    mut scored_for: Local<Vec<ColumnPos>>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    if pending.is_empty() && queue.0.is_empty() {
        return;
    }
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    // Nothing to score against, and a score is only stale once a player has moved to another
    // column: the walk is worth its cost then and wasted otherwise.
    if player_columns.is_empty() || *scored_for == *player_columns {
        return;
    }
    scored_for.clone_from(&*player_columns);

    let score = |column: ColumnPos| {
        crate::world::chunk::min_column_distance(&column, &player_columns)
            .clamp(0, Priority::MAX as i32) as Priority
    };
    pending.reprioritize(score);
    queue.0.reprioritize(score);
}

fn feed_light_edits(
    mut pending: ResMut<PendingEdits>,
    mut stages: MessageReader<SectionStageChanged>,
    blocks: Query<&ChunkBlocks>,
    players: Query<&Transform, With<Player>>,
    mut placed: MessageReader<BlockPlaced>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    // A column is not sent until its light is published, so the light queue has
    // to drain in the sender's order: the same distance to the nearest player
    // that the column scheduler already treats as a ticket level.
    let mut queue = |edit: Edit| {
        let distance = crate::world::chunk::min_column_distance(&edit.column(), &player_columns);
        pending.push_with_priority(edit, distance.clamp(0, Priority::MAX as i32) as Priority);
    };

    for change in stages.read() {
        if change.left() {
            queue(Edit::UnloadSection { pos: change.pos });
        } else if change.landed()
            && let Ok(section_blocks) = blocks.get(change.section)
        {
            queue(Edit::LoadSection {
                pos: change.pos,
                entity: change.section.to_bits(),
                blocks: Arc::clone(&section_blocks.0),
            });
        }
    }
    for placed in placed.read() {
        queue(Edit::SetBlock {
            pos: placed.block_pos,
            block: placed.new_state,
        });
    }
}

/// Hands the sky scan each column's surface, so a section holding no block of a
/// column costs it two seam tests rather than sixteen reads.
fn feed_column_surfaces(
    mut pending: ResMut<PendingEdits>,
    columns: Query<(&ColumnPosComponent, &SurfaceHeightmap), Changed<SurfaceHeightmap>>,
    players: Query<&Transform, With<Player>>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    if columns.is_empty() {
        return;
    }
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    for (pos, surface) in &columns {
        let distance = crate::world::chunk::min_column_distance(&pos.0, &player_columns);
        pending.push_with_priority(
            Edit::SetColumnSurface {
                column: pos.0,
                surface: Arc::new(surface.0.clone()),
            },
            distance.clamp(0, Priority::MAX as i32) as Priority,
        );
    }
}

/// Turns a published light change into a `ClientboundLightUpdate` carrying only
/// the rows that changed, for the players that already hold the column.
pub fn emit_light_updates(
    mut relit: MessageReader<SectionRelit>,
    sections: Query<(&SectionPos, &InDimension)>,
    column_indices: Query<&ColumnIndex>,
    observers: Query<&PlayerObservers>,
    anchors: Query<&HostAnchor>,
    codec_params: LightCodecParams,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut by_column: Local<FxHashMap<(Entity, ColumnPos), (Vec<Entity>, Vec<Entity>)>>,
) {
    for change in relit.read() {
        let Ok((pos, in_dim)) = sections.get(change.section) else {
            continue;
        };
        let rows = by_column
            .entry((in_dim.0, ColumnPos::from(*pos)))
            .or_default();
        if change.block && !rows.0.contains(&change.section) {
            rows.0.push(change.section);
        }
        if change.sky && !rows.1.contains(&change.section) {
            rows.1.push(change.section);
        }
    }

    for ((dim, column_pos), (block_rows, sky_rows)) in by_column.drain() {
        let Some(column_entity) = column_indices
            .get(dim)
            .ok()
            .and_then(|index| index.0.get(&column_pos).map(|slot| slot.entity))
        else {
            continue;
        };
        // The anchor, not the dimension world's player entity: the session
        // registry is keyed by anchor, and a target it cannot resolve is
        // dropped without a trace.
        let targets: SmallVec<[Entity; 8]> = observers
            .get(column_entity)
            .map(|held| {
                held.0
                    .iter()
                    .filter_map(|player| anchors.get(*player).ok().map(|anchor| anchor.0))
                    .collect()
            })
            .unwrap_or_default();
        if targets.is_empty() {
            continue;
        }

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::PlayerSet(targets),
            // A column's light is sent in full exactly once, so a shed delta is
            // never re-sent; Normal is the class `dispatch_encode` drops first.
            priority: PacketPriority::High,
            data: PacketPayload::LightUpdate {
                column: column_pos,
                light_data: build_delta_light_data(
                    column_entity,
                    &block_rows,
                    &sky_rows,
                    &codec_params,
                ),
            },
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}
