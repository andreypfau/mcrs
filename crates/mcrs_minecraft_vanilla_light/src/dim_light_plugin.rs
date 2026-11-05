use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLightUpdate;
use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use mcrs_engine_link::Placement;
use mcrs_minecraft_chunk::{BlockLight, LightLayer, SkyLight};
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine::column_production::ColumnProduction;
use mcrs_minecraft_engine::column_streams::ColumnStreams;
use mcrs_minecraft_engine::light_codec::{LightCodecParams, build_delta_light_data};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, ColumnLightSettled, LightGated, SectionRelit, SentLight,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::prelude::{Edit, LightBounds};
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::cold_light::ColdLightPlugin;
use crate::light_input::LightInput;
use crate::light_link::LightLink;
use crate::light_plugin::{LightPlugin, LightSet};
use crate::light_stage::player_priority;

use mcrs_minecraft_heightmap::SurfaceHeightmap;
use rustc_hash::{FxHashMap, FxHashSet};

use mcrs_minecraft_engine::bus::{PacketPayload, PacketPriority};

/// A dimension's side of the light engine: what it hands the light stage before intake, and
/// what it sends once the stage's light has been published.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DimLightSet {
    Feed,
    Emit,
}

pub(crate) struct DimLightPlugin {
    pub(crate) registry: Arc<LightRegistry>,
    pub(crate) bounds: LightBounds,
    pub(crate) sky: bool,
}

impl Plugin for DimLightPlugin {
    fn build(&self, app: &mut App) {
        if app.world().contains_resource::<ColumnProduction>() {
            app.add_plugins(ColdLightPlugin {
                registry: Arc::clone(&self.registry),
                bounds: self.bounds,
                sky: self.sky,
            });
        }
        app.add_plugins(LightPlugin {
            registry: Arc::clone(&self.registry),
            bounds: self.bounds,
            sky: self.sky,
            publish: FixedUpdate.intern(),
            placement: Placement::Thread,
        })
        .configure_sets(FixedUpdate, LightSet::Publish.in_set(TickSet::DomainInput))
        .configure_sets(Last, DimLightSet::Feed.before(LightSet::Intake))
        .configure_sets(
            Last,
            DimLightSet::Emit
                .after(DimLightSet::Feed)
                .before(LightSet::Intake),
        )
        // `Last` sees what the whole tick landed and placed, and hands it to the stage before
        // the tick ends.
        .add_systems(
            Last,
            (
                (
                    feed_light_edits,
                    reprioritize_light_work,
                    // A surface bound must never describe blocks the light world has not
                    // been handed yet.
                    feed_column_surfaces
                        .after(mcrs_minecraft_engine::heightmap::update_column_heightmaps),
                )
                    .chain()
                    .in_set(DimLightSet::Feed),
                emit_light_updates.in_set(DimLightSet::Emit),
            ),
        )
        .insert_resource(LightGated)
        .configure_sets(FixedUpdate, ColumnLightGate.in_set(TickSet::Project))
        .add_systems(FixedUpdate, await_light.in_set(ColumnLightGate));
    }
}

/// Hands the light stage each column that waits for its light. The stage answers once the
/// light around the column has settled, and [`publish_light`] passes the answer on.
fn await_light(mut awaiting: MessageReader<ColumnAwaitsLight>, mut light: ResMut<LightLink>) {
    light.await_columns(awaiting.read().map(|&ColumnAwaitsLight(column)| column));
}

/// Hands the light stage where the players stand once one of them has moved to another
/// column, so the waiting light work is re-scored against them.
fn reprioritize_light_work(
    mut light: ResMut<LightLink>,
    players: Query<&Transform, With<Player>>,
    mut scored_for: Local<Vec<ColumnPos>>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    if player_columns.is_empty() || *scored_for == *player_columns {
        return;
    }
    scored_for.clone_from(&*player_columns);
    light.send(LightInput::Players(player_columns.clone()));
}

fn feed_light_edits(
    mut light: ResMut<LightLink>,
    mut stages: MessageReader<SectionStageChanged>,
    blocks: Query<&ChunkBlocks>,
    players: Query<&Transform, With<Player>>,
    mut placed: MessageReader<BlockPlaced>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    let mut queue = |edit: Edit| {
        let priority = player_priority(edit.column(), &player_columns);
        light.push_with_priority(edit, priority);
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
    mut light: ResMut<LightLink>,
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
        light.push_with_priority(
            Edit::SetColumnSurface {
                column: pos.0,
                surface: Arc::new(surface.0.clone()),
            },
            player_priority(pos.0, &player_columns),
        );
    }
}

/// The light changes of one column that wait until its light is final.
#[derive(Default)]
pub struct HeldBack {
    block: Vec<Entity>,
    sky: Vec<Entity>,
    wait: Wait,
}

/// What the light of a held-back column waits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Wait {
    /// The stage has not lit every section of the column and of the eight around it.
    #[default]
    Ring,
    /// The stage was asked to answer once no work it holds reaches the column.
    Settle,
}

/// Turns a published light change into a `ClientboundLightUpdate` carrying only the rows that
/// changed, for a column that the muxer subscribes to.
///
/// A column at the outer ring of a hot area is lit here without its outer neighbours, so its
/// light goes out from the data layer and not from here. A change goes out only once the light
/// of the column is final: the stage has lit the eight around it and holds no work that reaches
/// it. Light lit while a neighbour is still on its way is light that the neighbour changes back,
/// and the clients would draw both changes. A row whose light is the [`SentLight`] that the
/// clients hold is no change to them.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn emit_light_updates(
    mut relit: MessageReader<SectionRelit>,
    mut settled: MessageReader<ColumnLightSettled>,
    mut light: ResMut<LightLink>,
    sections: Query<(&SectionPos, &InDimension)>,
    mut lit: Query<(&BlockLight, &SkyLight, Option<&mut SentLight>)>,
    column_indices: Query<(
        &ColumnIndex,
        &SectionIndex,
        &DimensionTypeConfig,
        &ColumnStreams,
        &ColumnLoading,
    )>,
    codec_params: LightCodecParams,
    mut changes: MessageWriter<ColumnChange>,
    mut held_back: Local<FxHashMap<(Entity, ColumnPos), HeldBack>>,
    mut settled_now: Local<FxHashSet<ColumnPos>>,
) {
    for change in relit.read() {
        let Ok((pos, in_dim)) = sections.get(change.section) else {
            continue;
        };
        let held = held_back
            .entry((in_dim.0, ColumnPos::from(*pos)))
            .or_default();
        if change.block && !held.block.contains(&change.section) {
            held.block.push(change.section);
        }
        if change.sky && !held.sky.contains(&change.section) {
            held.sky.push(change.section);
        }
    }
    settled_now.clear();
    settled_now.extend(settled.read().map(|&ColumnLightSettled(column)| column));

    held_back.retain(|&(dim, column_pos), held| {
        let Ok((index, section_index, rows, streams, loading)) = column_indices.get(dim) else {
            return false;
        };
        if !streams.is_subscribed(column_pos)
            || streams.published_light_of_this_tick(column_pos)
            || (loading.is_column_loaded(column_pos) && !loading.lights(column_pos))
        {
            return false;
        }
        let Some(column_entity) = index.0.get(&column_pos).map(|slot| slot.entity) else {
            return false;
        };
        match held.wait {
            Wait::Ring => {
                if ring_lit(index, section_index, rows, loading, &lit, column_pos) {
                    light.await_columns([column_pos]);
                    held.wait = Wait::Settle;
                }
                return true;
            }
            Wait::Settle if !settled_now.contains(&column_pos) => return true,
            Wait::Settle => {}
        }
        held.block
            .retain(|&section| note_sent(&mut lit, section, LightLayer::Block));
        held.sky
            .retain(|&section| note_sent(&mut lit, section, LightLayer::Sky));
        if held.block.is_empty() && held.sky.is_empty() {
            return false;
        }
        changes.write(ColumnChange {
            column: column_pos,
            // A column's light is sent in full exactly once, so a shed delta is
            // never re-sent; Normal is the class `dispatch_encode` drops first.
            priority: PacketPriority::High,
            payload: PacketPayload::LightUpdate(ClientboundLightUpdate {
                x: VarInt(column_pos.x),
                z: VarInt(column_pos.z),
                light_data: build_delta_light_data(
                    column_entity,
                    &held.block,
                    &held.sky,
                    &codec_params,
                ),
            }),
        });
        false
    });
}

/// Whether the stage has lit `column` and the eight around it. The light of a cell reaches one
/// column out, so until then the light of `column` is not final. Where the dimension lights
/// `column` every neighbour is on its way and each of its sections counts; elsewhere only the
/// sections the dimension holds count.
fn ring_lit(
    columns: &ColumnIndex,
    sections: &SectionIndex,
    rows: &DimensionTypeConfig,
    loading: &ColumnLoading,
    lit: &Query<(&BlockLight, &SkyLight, Option<&mut SentLight>)>,
    column: ColumnPos,
) -> bool {
    let first = rows.min_y >> SectionPos::BITS;
    let count = i32::try_from(rows.section_count).unwrap_or(0);
    let every_row = loading.lights(column);
    column
        .square(1)
        .filter(|near| every_row || columns.0.contains_key(near))
        .all(|near| {
            (first..first.saturating_add(count)).all(|y| {
                match sections.get(SectionPos::new(near.x, y, near.z)) {
                    Some(section) => lit.contains(section),
                    None => !every_row,
                }
            })
        })
}

fn note_sent(
    lit: &mut Query<(&BlockLight, &SkyLight, Option<&mut SentLight>)>,
    section: Entity,
    layer: LightLayer,
) -> bool {
    let Ok((block, sky, Some(mut sent))) = lit.get_mut(section) else {
        return true;
    };
    let (now, held) = match layer {
        LightLayer::Block => (&block.0, &mut sent.block),
        LightLayer::Sky => (&sky.0, &mut sent.sky),
    };
    if held.as_ref() == Some(now) {
        return false;
    }
    *held = Some(now.clone());
    true
}
