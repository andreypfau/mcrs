use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::prelude::{
    BlockLight, Edit, LightBounds, LightPlugin, LightSet, PendingEdits, Priority, SkyLight,
};
use mcrs_minecraft_protocol::light_codec::{LightCodecParams, build_delta_light_data};
use mcrs_voxel_math::{ChunkPos, ColumnPos};
use mcrs_voxel_world::entity::physics::Transform;
use mcrs_voxel_world::entity::player::Player;
use mcrs_voxel_world::session::PlayerSession;
use mcrs_voxel_world::world::dimension::InDimension;
use mcrs_voxel_world::world::lifecycle::markers::{ChunkFresh, ChunkLoaded};
use mcrs_voxel_world::world::storage::column::ColumnIndex;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::column_view::ColumnView;

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
        // `ChunkLoaded` and the block palette land in `FixedLast`, so `Last` is
        // the first schedule of the same tick that can see them.
        // After `Track`, so that a section respawned at a position the same tick
        // its predecessor died loads back after the unload rather than before it.
        .add_systems(
            Last,
            (
                feed_light_edits
                    .after(LightSet::Track)
                    .before(LightSet::Intake),
                emit_light_updates.after(LightSet::Publish),
            ),
        );
    }
}

fn feed_light_edits(
    mut pending: ResMut<PendingEdits>,
    loaded: Query<(&ChunkPos, &BlockPalette), (Added<ChunkLoaded>, With<ChunkFresh>)>,
    positions: Query<&ChunkPos>,
    players: Query<&Transform, With<Player>>,
    mut unloaded: RemovedComponents<ChunkLoaded>,
    mut placed: MessageReader<BlockPlaced>,
) {
    let player_columns: Vec<ColumnPos> = players
        .iter()
        .map(|at| ColumnPos::from(at.translation))
        .collect();
    // A column is not sent until its light is published, so the light queue has
    // to drain in the sender's order: the same distance to the nearest player
    // that the column scheduler already treats as a ticket level.
    let mut queue = |edit: Edit| {
        let distance = crate::world::chunk::min_column_distance(&edit.column(), &player_columns);
        pending.push_with_priority(edit, distance.clamp(0, Priority::MAX as i32) as Priority);
    };

    for (pos, blocks) in &loaded {
        queue(Edit::LoadSection {
            pos: *pos,
            blocks: Arc::new(blocks.clone()),
        });
    }
    for entity in unloaded.read() {
        if let Ok(pos) = positions.get(entity) {
            queue(Edit::UnloadSection { pos: *pos });
        }
    }
    for placed in placed.read() {
        queue(Edit::SetBlock {
            pos: placed.block_pos,
            block: placed.new_state,
        });
    }
}

/// Turns a published light change into a `ClientboundLightUpdate` carrying only
/// the rows that changed, for the players that already hold the column.
///
/// The scan is over every section entity, not only the changed ones: at view
/// distance 10 that is on the order of ten thousand tick comparisons per
/// dimension.
pub fn emit_light_updates(
    changed: Query<
        (
            Entity,
            &ChunkPos,
            &InDimension,
            Ref<BlockLight>,
            Ref<SkyLight>,
        ),
        Or<(Changed<BlockLight>, Changed<SkyLight>)>,
    >,
    column_indices: Query<&ColumnIndex>,
    views: Query<(Entity, &ColumnView)>,
    codec_params: LightCodecParams,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let mut by_column: FxHashMap<(Entity, ColumnPos), (Vec<Entity>, Vec<Entity>)> =
        FxHashMap::default();
    for (section, pos, in_dim, block, sky) in &changed {
        let rows = by_column
            .entry((in_dim.0, ColumnPos::from(*pos)))
            .or_default();
        if block.is_changed() {
            rows.0.push(section);
        }
        if sky.is_changed() {
            rows.1.push(section);
        }
    }

    for ((dim, column_pos), (block_rows, sky_rows)) in by_column {
        let Some(column_entity) = column_indices
            .get(dim)
            .ok()
            .and_then(|index| index.0.get(&column_pos).map(|slot| slot.entity))
        else {
            continue;
        };
        // Who holds the column is what the sender recorded, not what the area
        // of interest mirrors: that mirror is rebuilt from a player's movement,
        // so for a player standing still every column that finished loading
        // afterwards has an empty observer list and would never see a
        // correction to the light it was sent.
        let targets: SmallVec<[Entity; 8]> = views
            .iter()
            .filter(|(_, view)| view.sent_columns.contains(&column_pos))
            .map(|(player, _)| player)
            .collect();
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

/// Whether the light of a column is finished, neighbours included.
///
/// A column is sent once and its light travels with it, so anything the engine
/// still owes it would arrive too late. The neighbours count because a seam is
/// lit against whatever stands beside it: a column whose neighbour is still
/// being lit holds the darker edge that neighbour's work is about to repair.
pub fn light_settled_around(
    column_pos: ColumnPos,
    lighting: Option<&mcrs_minecraft_light::prelude::Lighting>,
    queue: Option<&mcrs_minecraft_light::prelude::LightWorkQueue>,
    epoch: Option<&mcrs_minecraft_light::prelude::LightEpoch>,
    pending: Option<&mcrs_minecraft_light::prelude::PendingEdits>,
) -> bool {
    use mcrs_minecraft_light::prelude::BlockBox;
    use mcrs_voxel_math::BlockPos;

    let Some(lighting) = lighting else {
        return true;
    };
    let neighbourhood = (-1..=1).flat_map(|dx| {
        (-1..=1).map(move |dz| ColumnPos::new(column_pos.x + dx, column_pos.z + dz))
    });
    for column in neighbourhood {
        if pending.is_some_and(|pending| pending.holds(column))
            || queue.is_some_and(|queue| queue.0.priority_of(column).is_some())
        {
            return false;
        }
    }
    let bounds = lighting.0.bounds();
    let area = BlockBox {
        min: BlockPos::new(
            (column_pos.x - 1) * 16,
            bounds.min_light_y(),
            (column_pos.z - 1) * 16,
        ),
        max: BlockPos::new(
            (column_pos.x + 2) * 16 - 1,
            bounds.max_light_y(),
            (column_pos.z + 2) * 16 - 1,
        ),
    };
    !epoch.is_some_and(|epoch| epoch.touches(area))
}
