use std::collections::VecDeque;

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::tick::TickSet;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::pending_spawn::PendingSpawn;
use crate::world::lifecycle::simulation::{
    SectionSimulation, SimulationDistance, raise_simulation_ticket, release_simulation_ticket,
    track_held_simulation_areas, track_simulation_tickets,
};
use crate::world::lifecycle::trace::{self, ColumnStage, ColumnTraceLog};
use crate::world::storage::chunk_spawn_set::ChunkSpawnSet;
use crate::world::storage::random_ticks::RandomTickIndex;
use crate::world::storage::section::{SectionBundle, SectionIndex};
use crate::world::storage::section_stage::{SectionStage, SectionStageChanged, SectionStages};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_voxel::SectionPos;

const MAX_DESPAWNS_PER_TICK: usize = MAX_SPAWNS_PER_TICK;
pub const MAX_SPAWNS_PER_TICK: usize = 4096;

pub struct TicketPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnCoolSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionEntitySet;

impl Plugin for TicketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SectionStageChanged>();
        app.init_resource::<Condemned>();
        app.init_resource::<SimulationDistance>();
        app.add_observer(raise_simulation_ticket);
        app.add_observer(release_simulation_ticket);
        app.configure_sets(
            FixedUpdate,
            (
                ChunkSpawnSet.in_set(TickSet::Project),
                SectionEntitySet.in_set(ChunkSpawnSet),
                ColumnCoolSet.in_set(SectionEntitySet),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                (track_simulation_tickets, track_held_simulation_areas)
                    .chain()
                    .in_set(ChunkSpawnSet)
                    .before(SectionEntitySet),
                (
                    release_sections.after(ColumnCoolSet),
                    despawn_chunks.run_if(on_message::<SectionStageChanged>.or_else(any_condemned)),
                    spawn_chunks,
                )
                    .chain()
                    .in_set(SectionEntitySet),
            ),
        );
    }
}

fn release_sections(
    mut dims: Populated<(&mut ColumnLoading, &SectionIndex)>,
    mut stages: SectionStages,
) {
    for (mut loading, section_index) in &mut dims {
        loading.release_pending(|pos| {
            let Some(section) = section_index.get(pos) else {
                return false;
            };
            if stages.get(section).is_none() {
                return true;
            }
            stages.set(section, SectionStage::Unloading);
            false
        });
    }
}

#[derive(Resource, Default)]
struct Condemned(VecDeque<Entity>);

fn any_condemned(condemned: Res<Condemned>) -> bool {
    !condemned.0.is_empty()
}

fn despawn_chunks(
    mut commands: Commands,
    mut changes: MessageReader<SectionStageChanged>,
    mut condemned: ResMut<Condemned>,
    mut dims: Query<(&mut SectionIndex, &mut ColumnLoading)>,
    sections: Query<(&SectionStage, &SectionPos, &InDimension)>,
    mut random_ticks: Option<ResMut<RandomTickIndex>>,
) {
    let condemned = &mut condemned.0;
    condemned.extend(
        changes
            .read()
            .filter(|change| change.to == SectionStage::Unloading)
            .map(|change| change.section),
    );
    let mut taken = 0usize;
    for _ in 0..condemned.len() {
        if taken >= MAX_DESPAWNS_PER_TICK {
            break;
        }
        let Some(section) = condemned.pop_front() else {
            break;
        };
        let Ok((stage, pos, dim)) = sections.get(section) else {
            continue;
        };
        if *stage != SectionStage::Unloading {
            continue;
        }
        let Ok((mut section_index, mut loading)) = dims.get_mut(dim.0) else {
            continue;
        };
        if loading.is_loaded(*pos) {
            loading.requeue(*pos);
            continue;
        }
        section_index.remove(*pos, random_ticks.as_deref_mut());
        commands.entity(section).try_despawn();
        taken += 1;
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the column trace log stamps wall-clock times only when the trace log exists, which a debug run enables"
    )
)]
pub fn spawn_chunks(
    mut dims: Populated<(
        Entity,
        &mut ColumnLoading,
        &mut SectionIndex,
        Option<&SectionSimulation>,
    )>,
    mut commands: Commands,
    mut stages: SectionStages,
    players: Query<(&Transform, &InDimension), With<Player>>,
    traces: Option<Res<ColumnTraceLog>>,
    mut centers: Local<Vec<SectionPos>>,
    mut keys_to_process: Local<Vec<PendingSpawn>>,
) {
    for (dim, mut loading, mut section_index, simulation) in dims.iter_mut() {
        if !loading.has_pending_spawns() {
            continue;
        }

        centers.clear();
        centers.extend(
            players
                .iter()
                .filter(|(_, in_dim)| in_dim.entity() == dim)
                .map(|(transform, _)| SectionPos::from(transform.translation)),
        );

        loading.take_nearest(&centers, MAX_SPAWNS_PER_TICK, &mut keys_to_process);

        for pending in keys_to_process.drain(..) {
            let pos = pending.pos();
            if !loading.is_loaded(pos) {
                continue;
            }

            let Some(section) = section_index.get(pos) else {
                trace::mark(traces.as_deref(), pos.into(), ColumnStage::Spawned);
                let mut section = commands.spawn(SectionBundle::new(InDimension(dim), pos));
                if let Some(simulation) = simulation {
                    simulation.mark_spawned(section.reborrow(), pos);
                }
                let section = section.id();
                section_index.insert(pos, section);
                stages.spawned(section, pos, dim, SectionStage::Loading);
                continue;
            };

            if stages.get(section) == Some(SectionStage::Unloading) {
                let back = if section_index.blocks(pos).is_some() {
                    SectionStage::Loaded
                } else {
                    SectionStage::Loading
                };
                stages.set(section, back);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use mcrs_voxel::ColumnPos;
    use std::ops::RangeInclusive;

    fn app_with_dimension(section_y: RangeInclusive<i32>) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(TicketPlugin);
        let dim = app
            .world_mut()
            .spawn((
                ColumnLoading::bounded(section_y),
                SectionSimulation::default(),
                SectionIndex::new(),
            ))
            .id();
        (app, dim)
    }

    fn loading(app: &mut App, dim: Entity) -> Mut<'_, ColumnLoading> {
        app.world_mut()
            .get_mut::<ColumnLoading>(dim)
            .expect("column loading")
    }

    fn section_index(app: &App, dim: Entity) -> &SectionIndex {
        app.world().get::<SectionIndex>(dim).expect("section index")
    }

    fn condemned_section(app: &mut App, dim: Entity, pos: SectionPos, blocks: bool) -> Entity {
        let mut section = app
            .world_mut()
            .spawn(SectionBundle::new(InDimension(dim), pos));
        section.insert(SectionStage::Unloading);
        let section = section.id();
        let mut index = app
            .world_mut()
            .get_mut::<SectionIndex>(dim)
            .expect("section index");
        index.insert(pos, section);
        if blocks {
            index.land(pos, section, crate::palette::air_section(), None, false);
        }
        app.world_mut().write_message(SectionStageChanged {
            section,
            pos,
            dim,
            from: Some(SectionStage::Loaded),
            to: SectionStage::Unloading,
        });
        section
    }

    #[test]
    fn a_loading_ticket_loads_every_section_of_its_column_and_two_rings_past_it() {
        let (mut app, dim) = app_with_dimension(-1..=1);
        loading(&mut app, dim).add_ticket(ColumnPos::new(0, 0));

        app.world_mut().run_schedule(FixedUpdate);

        let index = section_index(&app, dim);
        assert!(index.get(SectionPos::new(2, -1, -2)).is_some());
        assert!(index.get(SectionPos::new(3, 0, 0)).is_none());
        assert!(
            index.get(SectionPos::new(0, 2, 0)).is_none(),
            "nothing past the dimension's top"
        );
        assert_eq!(
            index.len(),
            5 * 5 * 3,
            "five columns across, three sections high"
        );
    }

    #[test]
    fn letting_go_of_a_ticket_takes_every_section_it_loaded() {
        let (mut app, dim) = app_with_dimension(-1..=1);
        let column = ColumnPos::new(0, 0);
        loading(&mut app, dim).add_ticket(column);
        app.world_mut().run_schedule(FixedUpdate);

        loading(&mut app, dim).remove_ticket(column);
        app.world_mut().run_schedule(FixedUpdate);

        assert!(section_index(&app, dim).is_empty());
    }

    #[test]
    fn a_ticket_raised_twice_holds_until_both_let_go() {
        let mut loading = ColumnLoading::bounded(0..=0);
        let column = ColumnPos::new(0, 0);
        let pos = SectionPos::new(0, 0, 0);
        loading.add_ticket(column);
        loading.add_ticket(column);

        loading.remove_ticket(column);
        assert!(loading.is_loaded(pos));

        loading.remove_ticket(column);
        assert!(!loading.is_loaded(pos));
        assert!(!loading.has_ticket(column));
    }

    #[test]
    fn a_backlog_spawns_the_sections_nearest_the_player_first() {
        let (mut app, dim) = app_with_dimension(0..=0);
        app.world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)));

        let far_columns = (MAX_SPAWNS_PER_TICK / 25) as i32 + 1;
        for x in 0..far_columns {
            loading(&mut app, dim).add_ticket(ColumnPos::new(1000 + 5 * x, 0));
        }
        loading(&mut app, dim).add_ticket(ColumnPos::new(0, 0));

        app.world_mut().run_schedule(FixedUpdate);

        let index = section_index(&app, dim);
        assert!(
            index.get(SectionPos::new(0, 0, 0)).is_some(),
            "the section under the player spawns even though it was asked for last"
        );
        let furthest = SectionPos::new(1000 + 5 * (far_columns - 1) + 2, 0, 2);
        assert!(
            index.get(furthest).is_none(),
            "the furthest section is the one left for the next tick"
        );
    }

    #[test]
    fn a_ticket_arriving_before_the_despawn_takes_the_section_back() {
        let (mut app, dim) = app_with_dimension(0..=0);
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, true);
        loading(&mut app, dim).add_ticket(ColumnPos::new(0, 0));

        app.world_mut().run_schedule(FixedUpdate);

        assert!(
            app.world().get_entity(dying).is_ok(),
            "the section that was asked for again is not taken"
        );
        assert_eq!(
            section_index(&app, dim).get(pos),
            Some(dying),
            "and it keeps its place in the index rather than being replaced"
        );
        assert_eq!(
            app.world().get::<SectionStage>(dying),
            Some(&SectionStage::Loaded)
        );
        let messages = app.world().resource::<Messages<SectionStageChanged>>();
        assert!(
            messages
                .get_cursor()
                .read(messages)
                .any(|change| change.section == dying && change.landed()),
            "it lands again, which is what counts it back into its column and its light"
        );
    }

    #[test]
    fn a_section_taken_back_before_its_blocks_arrived_is_generated_again() {
        let (mut app, dim) = app_with_dimension(0..=0);
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, false);
        loading(&mut app, dim).add_ticket(ColumnPos::new(0, 0));

        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(
            app.world().get::<SectionStage>(dying),
            Some(&SectionStage::Loading)
        );
    }

    #[test]
    fn a_ticket_outlives_the_chunk_it_arrived_too_late_for() {
        let (mut app, dim) = app_with_dimension(0..=0);
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, true);

        app.world_mut().run_schedule(FixedUpdate);
        assert!(app.world().get_entity(dying).is_err());

        loading(&mut app, dim).add_ticket(ColumnPos::new(0, 0));
        app.world_mut().run_schedule(FixedUpdate);
        let respawned = section_index(&app, dim)
            .get(pos)
            .expect("the ticket spawns a fresh section once the old one is gone");
        assert_ne!(respawned, dying);
    }
}
