use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use tracing::error;

use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::world::column_drain::DrainWake;
use mcrs_minecraft_engine_world::world::dimension_name::DimensionName;
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, SectionRelit};
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::epoch::PublishedLight;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::LightWorld;

use crate::from_light::FromLight;
use crate::intake_budget::IntakeBudget;
use crate::light_actor::LightActor;
use crate::light_budget::LightBudget;
use crate::light_state::LightState;
use crate::light_thread::LightThread;
use crate::to_light::ToLight;

/// The tick-loop steps, in the order they must run.
#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub enum LightSet {
    /// What the light actor published is written into [`BlockLight`] and [`SkyLight`].
    Publish,
    /// The edits pushed to the [`LightActor`] and any change to the budgets go to the actor.
    Intake,
}

/// Light computed by a light actor, a thread of its own per world. The actor stops when the
/// world drops its [`LightActor`].
pub struct LightPlugin {
    pub registry: Arc<LightRegistry>,
    pub bounds: LightBounds,
    /// Clear for a dimension with no sky, so the sky layer never gains a source.
    pub sky: bool,
}

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        let world = LightWorld::new(Arc::clone(&self.registry), self.bounds);
        let world = if self.sky { world } else { world.without_sky() };
        let wake = app.world().get_resource::<DrainWake>().cloned();
        app.insert_resource(LightThread::spawn(LightState::new(world), wake))
            .init_resource::<LightBudget>()
            .init_resource::<IntakeBudget>()
            .add_message::<SectionRelit>()
            .add_message::<ColumnLightSettled>()
            .add_systems(
                Last,
                (
                    publish_light.in_set(LightSet::Publish),
                    report_stopped_light
                        .run_if(light_stopped)
                        .in_set(LightSet::Publish),
                    (send_budgets, send_edits).chain().in_set(LightSet::Intake),
                )
                    .chain(),
            );
    }
}

/// Whether the light actor has replies that [`publish_light`] has not taken yet.
pub fn light_replied(actor: Res<LightActor>) -> bool {
    actor.has_replies()
}

fn light_stopped(actor: Res<LightActor>) -> bool {
    actor.stopped_unreported()
}

fn report_stopped_light(mut actor: ResMut<LightActor>, dimension: DimensionName) {
    error!(
        dimension = dimension.as_str(),
        "the light actor stopped, so this dimension gets no more light"
    );
    actor.report_stop();
}

/// Writes what the light actor published since the last run into the sections, and answers
/// the columns whose light settled.
pub fn publish_light(
    mut actor: ResMut<LightActor>,
    mut commands: Commands,
    mut lit: Query<(&mut BlockLight, &mut SkyLight)>,
    mut relit: MessageWriter<SectionRelit>,
    mut settled: MessageWriter<ColumnLightSettled>,
) {
    while let Some(reply) = actor.receive() {
        match reply {
            FromLight::Relit(sections) => {
                for published in sections {
                    write_light(published, &mut commands, &mut lit, &mut relit);
                }
            }
            FromLight::Settled(columns) => {
                for column in columns {
                    settled.write(ColumnLightSettled(column));
                }
            }
            FromLight::Idle { .. } => {}
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
