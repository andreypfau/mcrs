use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use mcrs_engine_link::Placement;
use mcrs_minecraft_engine::column_production::ColumnProduction;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, LightGated,
};
use mcrs_minecraft_engine_world::world::storage::column::ColumnPosComponent;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_heightmap::SurfaceHeightmap;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::prelude::{Edit, LightBounds};
use mcrs_voxel::ColumnPos;

use crate::cold_light::ColdLightPlugin;
use crate::light_link::LightLink;
use crate::light_plugin::{LightPlugin, LightSet};

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
        .add_systems(
            Last,
            (
                feed_light_edits
                    .run_if(on_message::<SectionStageChanged>.or_eager(on_message::<BlockPlaced>)),
                reprioritize_light_work,
                feed_column_surfaces
                    .after(mcrs_minecraft_engine::heightmap::update_column_heightmaps),
            )
                .chain()
                .in_set(DimLightFeedSet),
        )
        .insert_resource(LightGated)
        .configure_sets(FixedUpdate, ColumnLightGate.in_set(TickSet::Project))
        .add_systems(
            FixedUpdate,
            await_light
                .run_if(on_message::<ColumnAwaitsLight>)
                .in_set(ColumnLightGate),
        );
    }
}

fn await_light(mut awaiting: MessageReader<ColumnAwaitsLight>, mut light: ResMut<LightLink>) {
    light.await_columns(awaiting.read().map(|&ColumnAwaitsLight(column)| column));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "copies the player columns only when a player enters another column, and the copy moves to the light thread"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn reprioritize_light_work(
    mut light: ResMut<LightLink>,
    players: Populated<&Transform, With<Player>>,
) {
    light.update_players(players.iter().map(|at| ColumnPos::from(at.translation)));
}

fn feed_light_edits(
    mut light: ResMut<LightLink>,
    mut stages: MessageReader<SectionStageChanged>,
    section_indices: Query<&SectionIndex>,
    mut placed: MessageReader<BlockPlaced>,
) {
    let mut queue = |edit: Edit| light.push(edit);
    for change in stages.read() {
        if change.left() {
            queue(Edit::UnloadSection { pos: change.pos });
        } else if change.landed()
            && let Some(section_blocks) = section_indices
                .get(change.dim)
                .ok()
                .and_then(|index| index.blocks(change.pos))
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

fn feed_column_surfaces(
    mut light: ResMut<LightLink>,
    columns: Populated<(&ColumnPosComponent, &SurfaceHeightmap), Changed<SurfaceHeightmap>>,
) {
    for (pos, surface) in columns.iter() {
        light.push(Edit::SetColumnSurface {
            column: pos.0,
            surface: Arc::new(surface.0.clone()),
        });
    }
}
