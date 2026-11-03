//! The systems that spawn and release the sections the tickets load.

use std::collections::VecDeque;

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::palette::ChunkBlocks;
use crate::tick::TickSet;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::column_loading::{ColumnLoading, is_loaded};
use crate::world::lifecycle::simulation::{
    SectionSimulation, SimulationDistance, raise_simulation_ticket, release_simulation_ticket,
    track_simulation_tickets,
};
use crate::world::lifecycle::stage::{SectionStage, SectionStageChanged, SectionStages};
use crate::world::lifecycle::trace::{self, ColumnStage, ColumnTraceLog};
use crate::world::storage::section::{SectionBundle, SectionIndex};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_voxel::SectionPos;

/// Symmetric with the spawn cap: a pipeline that admits sections faster than it retires them
/// leaves dead sections sitting on the positions live ones are waiting for.
const MAX_DESPAWNS_PER_TICK: usize = MAX_SPAWNS_PER_TICK;
/// A view's row is 27 columns of 24 sections, and a column whose sections straddle two ticks
/// is sent a tick late, so the cap holds several rows. The view sizes its own intake against
/// this: raising more columns a tick than this can spawn grows the queue without bound.
pub const MAX_SPAWNS_PER_TICK: usize = 4096;

pub struct TicketPlugin;

/// Turns the tick's new tickets into chunk entities. It runs in `FixedUpdate` so the tickets a
/// view raised this tick become entities this tick, and whatever queues them can follow.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkSpawnSet;

/// The part of [`ChunkSpawnSet`] that spawns and despawns the sections the tickets load.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionEntitySet;

impl Plugin for TicketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SectionStageChanged>();
        app.init_resource::<SimulationDistance>();
        app.add_observer(raise_simulation_ticket);
        app.add_observer(release_simulation_ticket);
        app.configure_sets(
            FixedUpdate,
            (
                ChunkSpawnSet.in_set(TickSet::Project),
                SectionEntitySet.in_set(ChunkSpawnSet),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                track_simulation_tickets
                    .in_set(ChunkSpawnSet)
                    .before(SectionEntitySet),
                (
                    release_sections,
                    // Before the spawn, so a section it takes leaves the index in the same run
                    // that could hand its position to a fresh one.
                    despawn_chunks,
                    spawn_chunks,
                )
                    .chain()
                    .in_set(SectionEntitySet),
            ),
        );
    }
}

fn release_sections(
    mut dims: Query<(&mut ColumnLoading, &SectionIndex)>,
    mut stages: SectionStages,
) {
    for (mut loading, section_index) in &mut dims {
        let ColumnLoading {
            covered,
            section_y,
            pending_release,
            ..
        } = &mut *loading;
        pending_release.retain(|&pos| {
            if is_loaded(covered, section_y, pos) {
                return false;
            }
            let Some(section) = section_index.get(pos) else {
                return false;
            };
            // Spawned this run, so its stage lands with the next flush.
            if stages.get(section).is_none() {
                return true;
            }
            stages.set(section, SectionStage::Unloading);
            false
        });
    }
}

/// The queue outlives the tick because of the cap. An entry whose section was taken back
/// in the meantime no longer holds `Unloading` and is dropped when it comes up.
fn despawn_chunks(
    mut commands: Commands,
    mut changes: MessageReader<SectionStageChanged>,
    mut condemned: Local<VecDeque<Entity>>,
    mut dims: Query<(&mut SectionIndex, &mut ColumnLoading)>,
    sections: Query<(&SectionStage, &SectionPos, &InDimension)>,
) {
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
        // Loaded again while it was on its way out. `spawn_chunks` takes it back with the blocks
        // it already holds, which is the whole point of leaving it alone: taking it now would
        // mean generating the same terrain twice.
        if loading.is_loaded(*pos) {
            loading.pending_spawn.insert(*pos);
            continue;
        }
        section_index.remove(*pos);
        commands.entity(section).try_despawn();
        taken += 1;
    }
}

/// Squared distance to the nearest player, or zero when the dimension holds none: with nobody
/// to be near, every section is equally worth spawning.
fn nearest_player_distance_sq(pos: SectionPos, centers: &[SectionPos]) -> i32 {
    centers
        .iter()
        .map(|center| pos.distance_squared(**center))
        .min()
        .unwrap_or(0)
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn spawn_chunks(
    mut dims: Query<(
        Entity,
        &mut ColumnLoading,
        &mut SectionIndex,
        Option<&SectionSimulation>,
    )>,
    mut commands: Commands,
    has_blocks: Query<Has<ChunkBlocks>>,
    mut stages: SectionStages,
    players: Query<(&Transform, &InDimension), With<Player>>,
    traces: Option<Res<ColumnTraceLog>>,
    mut centers: Local<Vec<SectionPos>>,
    mut keys_to_process: Local<Vec<SectionPos>>,
) {
    for (dim, mut loading, mut section_index, simulation) in dims.iter_mut() {
        if loading.pending_spawn.is_empty() {
            continue;
        }

        // The backlog outlives the walk that raised it, so what is spawned first is chosen
        // against where the players stand now. Ticketing a section the player has since flown
        // past ahead of the one under their feet is what leaves a hole underneath them.
        centers.clear();
        centers.extend(
            players
                .iter()
                .filter(|(_, in_dim)| in_dim.entity() == dim)
                .map(|(transform, _)| SectionPos::from(transform.translation)),
        );

        keys_to_process.clear();
        keys_to_process.extend(loading.pending_spawn.iter().copied());
        if MAX_SPAWNS_PER_TICK < keys_to_process.len() {
            keys_to_process.select_nth_unstable_by_key(MAX_SPAWNS_PER_TICK, |pos| {
                nearest_player_distance_sq(*pos, &centers)
            });
            keys_to_process.truncate(MAX_SPAWNS_PER_TICK);
        }
        keys_to_process.sort_unstable_by_key(|pos| nearest_player_distance_sq(*pos, &centers));

        for pos in keys_to_process.drain(..) {
            loading.pending_spawn.swap_remove(&pos);
            if !loading.is_loaded(pos) {
                continue;
            }

            let Some(section) = section_index.get(pos) else {
                trace::mark(traces.as_deref(), pos.into(), ColumnStage::Spawned);
                let mut section = commands.spawn(SectionBundle::new(InDimension(dim), pos));
                if let Some(simulation) = simulation {
                    simulation.mark_spawned(&mut section, pos);
                }
                let section = section.id();
                section_index.insert(pos, section);
                stages.spawned(section, pos, dim, SectionStage::Loading);
                continue;
            };

            // A section on its way out still holds the blocks it was generated with, so taking
            // it back is far cheaper than despawning it and generating the same terrain again.
            // One condemned before its blocks arrived has nothing to keep and is generated again.
            if stages.get(section) == Some(SectionStage::Unloading) {
                let back = if has_blocks.get(section).unwrap_or(false) {
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
        if blocks {
            section.insert(crate::palette::air_section());
        }
        let section = section.id();
        app.world_mut()
            .get_mut::<SectionIndex>(dim)
            .expect("section index")
            .insert(pos, section);
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

    /// Two views raise the same ticket on a column they both see, and only the last one to let
    /// go may unload it.
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

    /// The backlog can hold more than a tick's worth of spawns, and what the player needs is
    /// what is under them, not what they asked for first.
    #[test]
    fn a_backlog_spawns_the_sections_nearest_the_player_first() {
        let (mut app, dim) = app_with_dimension(0..=0);
        app.world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)));

        // Raised first and far away, five columns apart so the loaded squares do not overlap:
        // insertion order alone would fill the whole tick.
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

    /// Flying back over ground you just left is the common case at a large render distance.
    /// The section still holds its blocks, so taking it back is a great deal cheaper than
    /// despawning it and generating the same terrain again.
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

    /// A ticket raised after the section is gone must still land, or the view believes it
    /// asked for something nothing will ever deliver.
    #[test]
    fn a_ticket_outlives_the_chunk_it_arrived_too_late_for() {
        let (mut app, dim) = app_with_dimension(0..=0);
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, true);

        // Nobody wants it, so it goes.
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
