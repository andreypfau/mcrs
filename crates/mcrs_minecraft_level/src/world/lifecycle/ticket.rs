//! Section tickets.

use std::collections::VecDeque;

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::entity::{Despawned, InTransit};
use crate::palette::ChunkBlocks;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::level::{ENTITY_TICKING_LEVEL, FULL_LEVEL, SectionLevels};
use crate::world::lifecycle::stage::{SectionStage, SectionStageChanged, SectionStages};
use crate::world::lifecycle::trace::{self, ColumnStage, ColumnTraceLog};
use crate::world::storage::section::{SectionBundle, SectionIndex};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::*;
use mcrs_minecraft_core::SectionPos;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

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

impl Plugin for TicketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SectionStageChanged>();
        app.init_resource::<SimulationDistance>();
        app.add_observer(release_simulation_ticket);
        app.add_systems(
            FixedUpdate,
            (
                track_simulation_tickets,
                propagate_section_levels,
                release_sections,
                // Before the spawn, so a section it takes leaves the index in the same run
                // that could hand its position to a fresh one.
                despawn_chunks,
                spawn_chunks,
            )
                .chain()
                .in_set(ChunkSpawnSet),
        );
    }
}

/// The reference's `simulation-distance` server property, counted in sections.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationDistance(pub u8);

impl Default for SimulationDistance {
    fn default() -> Self {
        Self(10)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TicketType {
    PlayerLoading,
    PlayerSimulation,
}

impl TicketType {
    pub fn loads(self) -> bool {
        matches!(self, Self::PlayerLoading)
    }

    pub fn simulates(self) -> bool {
        matches!(self, Self::PlayerSimulation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ticket {
    pub ty: TicketType,
    pub level: u8,
}

impl Ticket {
    pub const PLAYER_LOADING: Self = Self {
        ty: TicketType::PlayerLoading,
        level: ENTITY_TICKING_LEVEL,
    };

    pub fn player_simulation(distance: u8) -> Self {
        Self {
            ty: TicketType::PlayerSimulation,
            level: ENTITY_TICKING_LEVEL.saturating_sub(distance),
        }
    }
}

/// The tickets held on each section of a dimension.
///
/// Equal tickets are counted where the reference merges them: every view raises its own, so
/// one view letting go must not take a ticket another view still holds.
#[derive(Component, Debug, Default)]
pub struct SectionTickets {
    held: FxHashMap<SectionPos, SmallVec<[(Ticket, u32); 2]>>,
    changed: FxHashSet<SectionPos>,
}

impl SectionTickets {
    pub fn add(&mut self, pos: SectionPos, ticket: Ticket) {
        let held = self.held.entry(pos).or_default();
        match held.iter_mut().find(|(held, _)| *held == ticket) {
            Some((_, count)) => *count += 1,
            None => {
                held.push((ticket, 1));
                self.changed.insert(pos);
            }
        }
    }

    pub fn remove(&mut self, pos: SectionPos, ticket: Ticket) {
        let Some(held) = self.held.get_mut(&pos) else {
            return;
        };
        let Some(i) = held.iter().position(|(held, _)| *held == ticket) else {
            return;
        };
        held[i].1 -= 1;
        if held[i].1 == 0 {
            held.swap_remove(i);
            if held.is_empty() {
                self.held.remove(&pos);
            }
            self.changed.insert(pos);
        }
    }

    pub fn loading_level(&self, pos: SectionPos) -> Option<u8> {
        self.lowest(pos, TicketType::loads)
    }

    pub fn simulation_level(&self, pos: SectionPos) -> Option<u8> {
        self.lowest(pos, TicketType::simulates)
    }

    fn lowest(&self, pos: SectionPos, counts: fn(TicketType) -> bool) -> Option<u8> {
        self.held
            .get(&pos)?
            .iter()
            .filter(|(ticket, _)| counts(ticket.ty))
            .map(|(ticket, _)| ticket.level)
            .min()
    }
}

/// The simulation ticket a player holds, kept so it is handed back where it was raised.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationTicket {
    dim: Entity,
    pos: SectionPos,
    ticket: Ticket,
}

fn track_simulation_tickets(
    distance: Res<SimulationDistance>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &InDimension,
            Option<&mut SimulationTicket>,
        ),
        (With<Player>, Without<Despawned>, Without<InTransit>),
    >,
    departed: Query<
        Entity,
        (
            With<SimulationTicket>,
            Or<(With<Despawned>, With<InTransit>)>,
        ),
    >,
    mut dims: Query<&mut SectionTickets>,
    mut commands: Commands,
) {
    for player in &departed {
        commands.entity(player).remove::<SimulationTicket>();
    }
    let ticket = Ticket::player_simulation(distance.0);
    for (player, transform, in_dim, held) in &mut players {
        let wanted = SimulationTicket {
            dim: in_dim.0,
            pos: SectionPos::from(transform.translation),
            ticket,
        };
        if held.as_deref() == Some(&wanted) {
            continue;
        }
        if let Some(held) = held.as_deref()
            && let Ok(mut tickets) = dims.get_mut(held.dim)
        {
            tickets.remove(held.pos, held.ticket);
        }
        if let Ok(mut tickets) = dims.get_mut(wanted.dim) {
            tickets.add(wanted.pos, wanted.ticket);
        }
        match held {
            Some(mut held) => *held = wanted,
            None => {
                commands.entity(player).insert(wanted);
            }
        }
    }
}

fn release_simulation_ticket(
    remove: On<Remove, SimulationTicket>,
    held: Query<&SimulationTicket>,
    mut dims: Query<&mut SectionTickets>,
) {
    let Ok(held) = held.get(remove.event().entity) else {
        return;
    };
    if let Ok(mut tickets) = dims.get_mut(held.dim) {
        tickets.remove(held.pos, held.ticket);
    }
}

pub fn propagate_section_levels(
    mut dims: Query<(&mut SectionTickets, &mut SectionLevels)>,
    mut changed: Local<Vec<SectionPos>>,
    mut before: Local<FxHashMap<SectionPos, u8>>,
) {
    for (mut tickets, mut levels) in &mut dims {
        if tickets.changed.is_empty() {
            continue;
        }
        changed.extend(tickets.changed.drain());
        let tickets = &*tickets;
        let levels = &mut *levels;

        levels.loading.update(
            &changed,
            |pos| tickets.loading_level(pos).unwrap_or(u8::MAX),
            &mut before,
        );
        for (pos, old) in before.drain() {
            let was_loaded = old <= FULL_LEVEL;
            let is_loaded = levels.is_loaded(pos);
            if is_loaded && !was_loaded {
                levels.pending_release.remove(&pos);
                levels.pending_spawn.insert(pos);
            } else if was_loaded && !is_loaded {
                levels.pending_spawn.swap_remove(&pos);
                levels.pending_release.insert(pos);
            }
        }

        levels.simulation.update(
            &changed,
            |pos| tickets.simulation_level(pos).unwrap_or(u8::MAX),
            &mut before,
        );
        before.clear();
        changed.clear();
    }
}

fn release_sections(
    mut dims: Query<(&mut SectionLevels, &SectionIndex)>,
    mut stages: SectionStages,
) {
    for (mut levels, section_index) in &mut dims {
        let levels = &mut *levels;
        let loading = &levels.loading;
        levels.pending_release.retain(|&pos| {
            if loading.level(pos) <= FULL_LEVEL {
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
    mut dims: Query<(&mut SectionIndex, &mut SectionLevels)>,
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
        let Ok((mut section_index, mut levels)) = dims.get_mut(dim.0) else {
            continue;
        };
        // Loaded again while it was on its way out. `spawn_chunks` takes it back with the blocks
        // it already holds, which is the whole point of leaving it alone: taking it now would
        // mean generating the same terrain twice.
        if levels.is_loaded(*pos) {
            levels.pending_spawn.insert(*pos);
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

pub fn spawn_chunks(
    mut dims: Query<(Entity, &mut SectionLevels, &mut SectionIndex)>,
    mut commands: Commands,
    has_blocks: Query<Has<ChunkBlocks>>,
    mut stages: SectionStages,
    players: Query<(&Transform, &InDimension), With<Player>>,
    mut traces: Option<ResMut<ColumnTraceLog>>,
    mut centers: Local<Vec<SectionPos>>,
    mut keys_to_process: Local<Vec<SectionPos>>,
) {
    for (dim, mut levels, mut section_index) in dims.iter_mut() {
        if levels.pending_spawn.is_empty() {
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
        keys_to_process.extend(levels.pending_spawn.iter().copied());
        if MAX_SPAWNS_PER_TICK < keys_to_process.len() {
            keys_to_process.select_nth_unstable_by_key(MAX_SPAWNS_PER_TICK, |pos| {
                nearest_player_distance_sq(*pos, &centers)
            });
            keys_to_process.truncate(MAX_SPAWNS_PER_TICK);
        }
        keys_to_process.sort_unstable_by_key(|pos| nearest_player_distance_sq(*pos, &centers));

        for pos in keys_to_process.drain(..) {
            levels.pending_spawn.swap_remove(&pos);
            if !levels.is_loaded(pos) {
                continue;
            }

            let Some(section) = section_index.get(pos) else {
                trace::mark(&mut traces, pos.into(), ColumnStage::Spawned);
                let section = commands
                    .spawn(SectionBundle::new(InDimension(dim), pos))
                    .id();
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
    use crate::world::lifecycle::level::FullStatus;
    use bevy_ecs::message::Messages;

    fn app_with_dimension() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(TicketPlugin);
        let dim = app
            .world_mut()
            .spawn((
                SectionTickets::default(),
                SectionLevels::default(),
                SectionIndex::new(),
            ))
            .id();
        (app, dim)
    }

    fn raise_ticket(app: &mut App, dim: Entity, pos: SectionPos) {
        app.world_mut()
            .get_mut::<SectionTickets>(dim)
            .expect("section tickets")
            .add(pos, Ticket::PLAYER_LOADING);
    }

    fn levels(app: &App, dim: Entity) -> &SectionLevels {
        app.world()
            .get::<SectionLevels>(dim)
            .expect("section levels")
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
            section.insert(ChunkBlocks::default());
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
    fn a_loading_ticket_loads_its_section_and_two_rings_past_it() {
        let (mut app, dim) = app_with_dimension();
        let origin = SectionPos::new(0, 0, 0);
        raise_ticket(&mut app, dim, origin);

        app.world_mut().run_schedule(FixedUpdate);

        let levels = levels(&app, dim);
        assert_eq!(levels.status(origin), FullStatus::EntityTicking);
        assert_eq!(
            levels.status(SectionPos::new(1, 1, -1)),
            FullStatus::BlockTicking
        );
        assert_eq!(levels.status(SectionPos::new(2, -2, 0)), FullStatus::Full);
        assert!(!levels.is_loaded(SectionPos::new(0, 3, 0)));
        assert_eq!(
            section_index(&app, dim).len(),
            125,
            "the cube five sections wide around the ticket spawns"
        );
    }

    #[test]
    fn letting_go_of_a_ticket_takes_every_section_it_loaded() {
        let (mut app, dim) = app_with_dimension();
        let origin = SectionPos::new(0, 0, 0);
        raise_ticket(&mut app, dim, origin);
        app.world_mut().run_schedule(FixedUpdate);

        app.world_mut()
            .get_mut::<SectionTickets>(dim)
            .expect("section tickets")
            .remove(origin, Ticket::PLAYER_LOADING);
        app.world_mut().run_schedule(FixedUpdate);

        assert!(section_index(&app, dim).is_empty());
    }

    /// Two views raise the same ticket on a section they both see, and only the last one to let
    /// go may unload it.
    #[test]
    fn a_ticket_raised_twice_holds_until_both_let_go() {
        let mut tickets = SectionTickets::default();
        let pos = SectionPos::new(0, 0, 0);
        tickets.add(pos, Ticket::PLAYER_LOADING);
        tickets.add(pos, Ticket::PLAYER_LOADING);

        tickets.remove(pos, Ticket::PLAYER_LOADING);
        assert_eq!(tickets.loading_level(pos), Some(ENTITY_TICKING_LEVEL));

        tickets.remove(pos, Ticket::PLAYER_LOADING);
        assert_eq!(tickets.loading_level(pos), None);
    }

    /// The backlog can hold more than a tick's worth of spawns, and what the player needs is
    /// what is under them, not what they asked for first.
    #[test]
    fn a_backlog_spawns_the_sections_nearest_the_player_first() {
        let (mut app, dim) = app_with_dimension();
        app.world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)));

        let under_the_player = SectionPos::new(0, 0, 0);
        // Raised first and far away, so insertion order alone would fill the whole tick.
        for x in 0..=MAX_SPAWNS_PER_TICK as i32 {
            raise_ticket(&mut app, dim, SectionPos::new(1000 + x, 0, 0));
        }
        raise_ticket(&mut app, dim, under_the_player);

        app.world_mut().run_schedule(FixedUpdate);

        let index = section_index(&app, dim);
        assert!(
            index.get(under_the_player).is_some(),
            "the section under the player spawns even though it was asked for last"
        );
        assert!(
            index
                .get(SectionPos::new(1000 + MAX_SPAWNS_PER_TICK as i32, 0, 0))
                .is_none(),
            "the furthest section is the one left for the next tick"
        );
    }

    /// Flying back over ground you just left is the common case at a large render distance.
    /// The section still holds its blocks, so taking it back is a great deal cheaper than
    /// despawning it and generating the same terrain again.
    #[test]
    fn a_ticket_arriving_before_the_despawn_takes_the_section_back() {
        let (mut app, dim) = app_with_dimension();
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, true);
        raise_ticket(&mut app, dim, pos);

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
        let (mut app, dim) = app_with_dimension();
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, false);
        raise_ticket(&mut app, dim, pos);

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
        let (mut app, dim) = app_with_dimension();
        let pos = SectionPos::new(0, 0, 0);
        let dying = condemned_section(&mut app, dim, pos, true);

        // Nobody wants it, so it goes.
        app.world_mut().run_schedule(FixedUpdate);
        assert!(app.world().get_entity(dying).is_err());

        raise_ticket(&mut app, dim, pos);
        app.world_mut().run_schedule(FixedUpdate);
        let respawned = section_index(&app, dim)
            .get(pos)
            .expect("the ticket spawns a fresh section once the old one is gone");
        assert_ne!(respawned, dying);
    }

    #[test]
    fn a_player_holds_one_simulation_ticket_that_follows_them_and_goes_when_they_do() {
        let (mut app, dim) = app_with_dimension();
        app.insert_resource(SimulationDistance(2));
        let player = app
            .world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)))
            .id();

        app.world_mut().run_schedule(FixedUpdate);
        let levels_now = levels(&app, dim);
        assert!(levels_now.is_entity_ticking(SectionPos::new(2, 0, -2)));
        assert!(!levels_now.is_entity_ticking(SectionPos::new(3, 0, 0)));
        assert!(levels_now.is_block_ticking(SectionPos::new(3, 0, 0)));

        app.world_mut()
            .get_mut::<Transform>(player)
            .expect("transform")
            .translation
            .x = 16.0 * 10.0;
        app.world_mut().run_schedule(FixedUpdate);
        let levels_now = levels(&app, dim);
        assert!(!levels_now.is_block_ticking(SectionPos::new(0, 0, 0)));
        assert!(levels_now.is_entity_ticking(SectionPos::new(10, 0, 0)));

        app.world_mut().entity_mut(player).insert(Despawned);
        app.world_mut().run_schedule(FixedUpdate);
        assert!(!levels(&app, dim).is_block_ticking(SectionPos::new(10, 0, 0)));
    }
}
