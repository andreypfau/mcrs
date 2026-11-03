use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::InternedScheduleLabel;
use rustc_hash::FxHashMap;

use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, SectionRelit};
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::epoch::PublishedLight;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::LightWorld;
use mcrs_voxel::ColumnPos;

use crate::from_light::FromLight;
use crate::intake_budget::IntakeBudget;
use crate::light_actor::LightActor;
use crate::light_budget::LightBudget;
use crate::light_state::LightState;
use crate::light_thread::LightThread;
use crate::to_light::ToLight;

/// The tick-loop steps.
#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub enum LightSet {
    /// What the light actor published is written into [`BlockLight`] and [`SkyLight`], in the
    /// schedule [`LightPlugin::publish`] names.
    Publish,
    /// The edits pushed to the [`LightActor`] and any change to the budgets go to the actor, in
    /// `Last`.
    Intake,
}

/// Light computed by a light actor, a thread of its own per world. The actor stops when the
/// world drops its [`LightActor`].
pub struct LightPlugin {
    pub registry: Arc<LightRegistry>,
    pub bounds: LightBounds,
    /// Clear for a dimension with no sky, so the sky layer never gains a source.
    pub sky: bool,
    /// The schedule the light lands in. A world whose simulation reads the light names the
    /// start of its tick, so the light changes only at a tick boundary.
    pub publish: InternedScheduleLabel,
}

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        let world = LightWorld::new(Arc::clone(&self.registry), self.bounds);
        let world = if self.sky { world } else { world.without_sky() };
        app.insert_resource(LightThread::spawn(LightState::new(world)))
            .init_resource::<LightBudget>()
            .init_resource::<IntakeBudget>()
            .add_message::<SectionRelit>()
            .add_message::<ColumnLightSettled>()
            .configure_sets(Last, LightSet::Publish.before(LightSet::Intake))
            .add_systems(
                self.publish,
                publish_light
                    .run_if(light_replied)
                    .in_set(LightSet::Publish),
            )
            .add_systems(
                Last,
                (send_budgets, send_edits).chain().in_set(LightSet::Intake),
            );
    }
}

/// Whether the light actor has replies that [`publish_light`] has not taken yet.
pub fn light_replied(actor: Res<LightActor>) -> bool {
    actor.has_replies()
}

/// Writes what the light actor published since the last run into the sections, and answers
/// the columns whose light settled.
///
/// The actor's replies are cut into batches by its thread timing, so they are folded into one
/// latest answer per section and written in section order: what lands, and the messages it
/// raises, do not depend on how the replies were cut.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn publish_light(
    mut actor: ResMut<LightActor>,
    mut commands: Commands,
    mut lit: Query<(&mut BlockLight, &mut SkyLight)>,
    mut relit: MessageWriter<SectionRelit>,
    mut settled: MessageWriter<ColumnLightSettled>,
    mut latest: Local<FxHashMap<u64, PublishedLight>>,
    mut sections: Local<Vec<PublishedLight>>,
    mut columns: Local<Vec<ColumnPos>>,
) {
    while let Some(reply) = actor.receive() {
        match reply {
            FromLight::Relit(published) => {
                for light in published {
                    fold_light(&mut latest, light);
                }
            }
            FromLight::Settled(answered) => columns.extend(answered),
            FromLight::Idle { .. } => {}
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

fn send_budgets(light: Res<LightBudget>, intake: Res<IntakeBudget>, mut actor: ResMut<LightActor>) {
    if light.is_changed() {
        actor.send(ToLight::LightBudget(*light));
    }
    if intake.is_changed() {
        actor.send(ToLight::IntakeBudget(*intake));
    }
}

fn send_edits(mut actor: ResMut<LightActor>) {
    actor.send_edits();
}
