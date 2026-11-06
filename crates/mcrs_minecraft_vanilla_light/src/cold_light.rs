use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::prelude::{IntoScheduleConfigs, Query, ResMut, Resource, With};
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine::cold_light_edit::ColdLightEdit;
use mcrs_minecraft_engine::column_light::{ColdStageLandSet, ColumnLight, LitSection};
use mcrs_minecraft_engine::column_production::ColumnProduction;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::{Edit, LightWorld};
use mcrs_voxel::ColumnPos;

use crate::light_link::LightLink;
use crate::light_output::LightOutput;
use crate::light_stage::LightStage;

/// The light stage of cold space: a second light stage that lights the columns the data layer
/// produces, from the blocks the data layer hands it. It publishes into no section of the
/// dimension, so what the dimension simulates does not depend on when it is done.
pub(crate) struct ColdLightPlugin {
    pub(crate) registry: Arc<LightRegistry>,
    pub(crate) bounds: LightBounds,
    pub(crate) sky: bool,
}

/// The near end of the link to the light stage of cold space.
#[derive(Resource)]
pub struct ColdLightLink(LightLink);

impl ColdLightLink {
    /// Waits until the stage has answered everything handed to it so far, and hands out none
    /// of it.
    ///
    /// For tests and tools only, to make the answers land at the same tick in every run.
    pub async fn answered(&mut self) {
        self.0.answered().await;
    }
}

impl Plugin for ColdLightPlugin {
    fn build(&self, app: &mut App) {
        let world = LightWorld::new(Arc::clone(&self.registry), self.bounds);
        let world = if self.sky { world } else { world.without_sky() };
        let link = Link::new(LightStage::awaited_only(world), Placement::Thread);
        app.insert_resource(ColdLightLink(LightLink::new(link)))
            .add_systems(FixedUpdate, land_cold_light.in_set(ColdStageLandSet))
            .add_systems(Last, send_cold_light);
    }
}

/// Hands the light stage of cold space the edits and the awaited columns of the data layer,
/// the columns nearest a player first.
fn send_cold_light(
    mut production: ResMut<ColumnProduction>,
    mut light: ResMut<ColdLightLink>,
    players: Query<&Transform, With<Player>>,
) {
    let light = &mut light.0;
    light.update_players(players.iter().map(|at| ColumnPos::from(at.translation)));
    for edit in production.take_light_edits() {
        let edit = match edit {
            ColdLightEdit::Load { pos, blocks } => Edit::LoadSection {
                pos,
                entity: 0,
                blocks: blocks.0,
            },
            ColdLightEdit::Unload { pos } => Edit::UnloadSection { pos },
            ColdLightEdit::SetBlock { pos, state } => Edit::SetBlock { pos, block: state },
        };
        light.push(edit);
    }
    light.await_columns(production.take_light_awaits());
    light.send_edits();
}

/// Hands the data layer the light of each column whose light settled.
fn land_cold_light(mut production: ResMut<ColumnProduction>, mut light: ResMut<ColdLightLink>) {
    for output in light.0.outputs() {
        let LightOutput::Lit(columns) = output else {
            continue;
        };
        for column in columns {
            production.lit(ColumnLight {
                column: column.column,
                sections: column
                    .sections
                    .into_iter()
                    .map(|section| LitSection {
                        y: section.pos.y,
                        block_light: BlockLight(section.block_light.unwrap_or_default()),
                        sky_light: SkyLight(section.sky_light.unwrap_or_default()),
                    })
                    .collect(),
            });
        }
    }
}
