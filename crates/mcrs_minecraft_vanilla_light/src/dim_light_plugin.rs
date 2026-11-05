use std::mem;
use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use mcrs_engine_link::Placement;
use mcrs_minecraft_engine::column_production::ColumnProduction;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, LightGated,
};
use mcrs_minecraft_engine_world::world::storage::column::ColumnPosComponent;
use mcrs_minecraft_heightmap::SurfaceHeightmap;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::prelude::{Edit, LightBounds};
use mcrs_voxel::ColumnPos;

use crate::cold_light::ColdLightPlugin;
use crate::light_input::LightInput;
use crate::light_link::LightLink;
use crate::light_plugin::{LightPlugin, LightSet};
use crate::light_stage::player_priority;

/// What a dimension hands its light stage before intake.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DimLightFeedSet;

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
        .configure_sets(Last, DimLightFeedSet.before(LightSet::Intake))
        // `Last` sees what the whole tick landed and placed, and hands it to the stage before
        // the tick ends.
        .add_systems(
            Last,
            (
                feed_light_edits,
                reprioritize_light_work,
                // A surface bound must never describe blocks the light world has not
                // been handed yet.
                feed_column_surfaces
                    .after(mcrs_minecraft_engine::heightmap::update_column_heightmaps),
            )
                .chain()
                .in_set(DimLightFeedSet),
        )
        .insert_resource(LightGated)
        .configure_sets(FixedUpdate, ColumnLightGate.in_set(TickSet::Project))
        .add_systems(FixedUpdate, await_light.in_set(ColumnLightGate));
    }
}

/// Hands the light stage each column that waits for its light. The stage answers once the
/// light of the column is final, and [`publish_light`](crate::publish_light) passes the answer
/// on.
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
    mem::swap(&mut *scored_for, &mut *player_columns);
    light.send(LightInput::Players(scored_for.clone()));
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
