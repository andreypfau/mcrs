use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::InternedScheduleLabel;
use mcrs_engine_link::{Link, Placement};
use rustc_hash::FxHashMap;

use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, SectionRelit};
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::epoch::PublishedLight;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::LightWorld;
use mcrs_voxel::ColumnPos;

use crate::light_budget::LightBudget;
use crate::light_input::LightInput;
use crate::light_link::LightLink;
use crate::light_output::LightOutput;
use crate::light_stage::LightStage;

/// The tick-loop steps.
#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub enum LightSet {
    /// What the light stage published is written into [`BlockLight`] and [`SkyLight`], in the
    /// schedule [`LightPlugin::publish`] names.
    Publish,
    /// The edits pushed to the [`LightLink`] and any change to the budgets go to the stage, in
    /// `Last`.
    Intake,
}

/// Light computed by a light stage, one per world, at the far end of a [`LightLink`]. The stage
/// stops when the world drops its [`LightLink`].
pub struct LightPlugin {
    pub registry: Arc<LightRegistry>,
    pub bounds: LightBounds,
    /// Clear for a dimension with no sky, so the sky layer never gains a source.
    pub sky: bool,
    /// The schedule the light lands in. A world whose simulation reads the light names the
    /// start of its tick, so the light changes only at a tick boundary.
    pub publish: InternedScheduleLabel,
    /// Where the stage runs. [`Placement::Inline`] lights everything handed to the stage in
    /// [`LightSet::Publish`], on the thread of the world.
    pub placement: Placement,
}

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        let world = LightWorld::new(Arc::clone(&self.registry), self.bounds);
        let world = if self.sky { world } else { world.without_sky() };
        let link = Link::new(LightStage::new(world), self.placement);
        app.insert_resource(LightLink::new(link))
            .init_resource::<LightBudget>()
            .add_message::<SectionRelit>()
            .add_message::<ColumnLightSettled>()
            .configure_sets(Last, LightSet::Publish.before(LightSet::Intake))
            .add_systems(self.publish, publish_light.in_set(LightSet::Publish))
            .add_systems(
                Last,
                (send_budgets, send_edits).chain().in_set(LightSet::Intake),
            );
    }
}

/// Writes what the light stage published since the last run into the sections, and answers
/// the columns whose light settled.
///
/// The outputs of the stage are cut into batches by its thread timing, so they are folded into
/// one latest answer per section and written in section order: what lands, and the messages it
/// raises, do not depend on how the outputs were cut.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn publish_light(
    mut light: ResMut<LightLink>,
    mut commands: Commands,
    mut lit: Query<(&mut BlockLight, &mut SkyLight)>,
    mut relit: MessageWriter<SectionRelit>,
    mut settled: MessageWriter<ColumnLightSettled>,
    mut latest: Local<FxHashMap<u64, PublishedLight>>,
    mut sections: Local<Vec<PublishedLight>>,
    mut columns: Local<Vec<ColumnPos>>,
) {
    for output in light.outputs() {
        match output {
            LightOutput::Relit(published) => {
                for published in published {
                    fold_light(&mut latest, published);
                }
            }
            LightOutput::Settled(answered) => columns.extend(answered),
            LightOutput::Lit(_) => {}
        }
    }
    sections.extend(latest.drain().map(|(_, light)| light));
    sections.sort_unstable_by_key(|light| light.entity);
    for published in sections.drain(..) {
        write_light(published, &mut commands, &mut lit, &mut relit);
    }
    columns.sort_unstable();
    columns.dedup();
    for column in columns.drain(..) {
        settled.write(ColumnLightSettled(column));
    }
}

/// A later layer replaces an earlier one; a layer the later answer leaves out stays.
fn fold_light(latest: &mut FxHashMap<u64, PublishedLight>, light: PublishedLight) {
    match latest.get_mut(&light.entity) {
        Some(earlier) => {
            if light.block_light.is_some() {
                earlier.block_light = light.block_light;
            }
            if light.sky_light.is_some() {
                earlier.sky_light = light.sky_light;
            }
        }
        None => {
            latest.insert(light.entity, light);
        }
    }
}

fn write_light(
    published: PublishedLight,
    commands: &mut Commands,
    lit: &mut Query<(&mut BlockLight, &mut SkyLight)>,
    relit: &mut MessageWriter<SectionRelit>,
) {
    let section = Entity::from_bits(published.entity);
    match lit.get_mut(section) {
        // Only a real change should wake whatever rebuilds meshes, and the
        // epoch already decided that: a layer it hands back is one whose
        // answer moved, so assigning it wakes nothing that should have slept.
        Ok((mut block, mut sky)) => {
            relit.write(SectionRelit {
                section,
                block: published.block_light.is_some(),
                sky: published.sky_light.is_some(),
            });
            if let Some(light) = published.block_light {
                block.0 = light;
            }
            if let Some(light) = published.sky_light {
                sky.0 = light;
            }
        }
        // A section reaches its entity for the first time with both layers
        // in hand, because nothing was published for it to match against.
        Err(_) => {
            if let Ok(mut entity) = commands.get_entity(section) {
                entity.insert((
                    BlockLight(published.block_light.unwrap_or_default()),
                    SkyLight(published.sky_light.unwrap_or_default()),
                ));
                relit.write(SectionRelit {
                    section,
                    block: true,
                    sky: true,
                });
            }
        }
    }
}

fn send_budgets(budget: Res<LightBudget>, mut light: ResMut<LightLink>) {
    if budget.is_changed() {
        light.send(LightInput::LightBudget(*budget));
    }
}

fn send_edits(mut light: ResMut<LightLink>) {
    light.send_edits();
}
