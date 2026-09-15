//! Section tickets.

use std::collections::VecDeque;

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::palette::ChunkBlocks;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::stage::{SectionStage, SectionStageChanged, SectionStages};
use crate::world::lifecycle::trace::{self, ColumnStage};
use crate::world::storage::section::SectionBundle;
use crate::world::storage::section::SectionIndex;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use indexmap::IndexMap;
use mcrs_minecraft_core::SectionPos;
use rustc_hash::{FxBuildHasher, FxHashMap};

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
        app.add_systems(FixedUpdate, spawn_chunks.in_set(ChunkSpawnSet));
        app.add_systems(
            FixedUpdate,
            (
                // Before the spawn, so a section it takes leaves the index in the same run
                // that could hand its position to a fresh one.
                despawn_chunks.before(ChunkSpawnSet),
                remove_tickets_from_chunks,
            ),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct Ticket {
    pub kind: TicketKind,
    pub ticks_left: i64,
}

impl Ticket {
    pub fn new(kind: TicketKind) -> Self {
        Self {
            kind,
            ticks_left: kind.timeout().unwrap_or(0) as i64,
        }
    }

    pub fn decrease_ticks_left(&mut self) {
        if self.kind.timeout().is_some() {
            self.ticks_left -= 1;
        }
    }

    pub fn is_expired(&self) -> bool {
        if self.kind.timeout().is_some() {
            self.ticks_left < 0
        } else {
            false
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TicketKind {
    PlayerSimulation,
    Forced,
    #[default]
    Unknown,
}

impl TicketKind {
    pub fn timeout(&self) -> Option<u64> {
        match self {
            TicketKind::PlayerSimulation => None,
            TicketKind::Forced => None,
            TicketKind::Unknown => Some(1),
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct ChunkTicketsCommands {
    /// Queued ticket adds. The order they were raised in says nothing about what the player
    /// needs now, so `spawn_chunks` picks the nearest rather than the oldest; the map is
    /// insertion-ordered only so a tick's spawns are reproducible.
    add_tickets: IndexMap<SectionPos, Vec<Ticket>, FxBuildHasher>,
    remove_tickets: FxHashMap<SectionPos, Vec<TicketKind>>,
}

#[derive(Component, Deref, DerefMut)]
pub struct SectionTicketHolder(pub Vec<Ticket>);

impl SectionTicketHolder {
    pub fn add(&mut self, ticket: Ticket) {
        self.0.push(ticket);
        self.0.sort_unstable_by(|a, b| b.cmp(a));
    }

    pub fn add_all(&mut self, tickets: Vec<Ticket>) {
        self.0.extend(tickets);
        self.0.sort_unstable_by(|a, b| b.cmp(a));
    }

    pub fn remove(&mut self, ticket_kind: TicketKind) {
        if let Some(i) = self.0.iter().position(|t| t.kind == ticket_kind) {
            self.0.swap_remove(i);
        }
    }
}

impl ChunkTicketsCommands {
    pub fn add_ticket(&mut self, chunk_pos: SectionPos, ticket: Ticket) {
        self.add_tickets.entry(chunk_pos).or_default().push(ticket);
    }

    /// Tickets raised for a section that has not been spawned yet.
    pub fn queued_tickets(&self, chunk_pos: SectionPos) -> &[Ticket] {
        self.add_tickets
            .get(&chunk_pos)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// A ticket still queued is cancelled where it waits; one already handed to a
    /// spawned chunk has to be taken off that chunk's holder instead, or the chunk
    /// keeps a ticket nobody holds and never unloads.
    pub fn remove_ticket(&mut self, chunk_pos: SectionPos, ticket_kind: TicketKind) {
        if let Some(tickets) = self.add_tickets.get_mut(&chunk_pos)
            && let Some(i) = tickets.iter().position(|t| t.kind == ticket_kind)
        {
            tickets.swap_remove(i);
            // A request nobody holds any more has to leave the queue, not stay as an empty
            // one: `spawn_chunks` would honour it, and the section it raised would be
            // condemned the same tick for holding no ticket. The queue would then never
            // shrink and the churn would crowd out the sections somebody is waiting for.
            if tickets.is_empty() {
                self.add_tickets.swap_remove(&chunk_pos);
            }
            return;
        }
        self.remove_tickets
            .entry(chunk_pos)
            .or_default()
            .push(ticket_kind);
    }
}

/// The queue outlives the tick because of the cap. An entry whose section was taken back
/// in the meantime no longer holds `Unloading` and is dropped when it comes up.
fn despawn_chunks(
    mut commands: Commands,
    mut changes: MessageReader<SectionStageChanged>,
    mut condemned: Local<VecDeque<Entity>>,
    mut dims: Query<(&mut SectionIndex, &ChunkTicketsCommands)>,
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
        let Ok((mut section_index, tickets)) = dims.get_mut(**dim) else {
            continue;
        };
        // Somebody asked for this section again while it was on its way out. `spawn_chunks`
        // brings it back with the blocks it already holds, which is the whole point of
        // leaving it alone: taking it now would mean generating the same terrain twice.
        if !tickets.queued_tickets(*pos).is_empty() {
            condemned.push_back(section);
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
    mut dims: Query<(Entity, &mut ChunkTicketsCommands, &mut SectionIndex)>,
    mut commands: Commands,
    mut holders: Query<(&mut SectionTicketHolder, Has<ChunkBlocks>)>,
    mut stages: SectionStages,
    players: Query<(&Transform, &InDimension), With<Player>>,
) {
    for (dim, mut chunk_tickets, mut chunk_index) in dims.iter_mut() {
        if chunk_tickets.add_tickets.is_empty() {
            continue;
        }

        // The backlog outlives the walk that raised it, so what is spawned first is chosen
        // against where the players stand now. Ticketing a section the player has since flown
        // past ahead of the one under their feet is what leaves a hole underneath them.
        let centers: Vec<SectionPos> = players
            .iter()
            .filter(|(_, in_dim)| in_dim.entity() == dim)
            .map(|(transform, _)| SectionPos::from(transform.translation))
            .collect();

        let mut keys_to_process: Vec<SectionPos> =
            chunk_tickets.add_tickets.keys().copied().collect();
        if MAX_SPAWNS_PER_TICK < keys_to_process.len() {
            keys_to_process.select_nth_unstable_by_key(MAX_SPAWNS_PER_TICK, |pos| {
                nearest_player_distance_sq(*pos, &centers)
            });
            keys_to_process.truncate(MAX_SPAWNS_PER_TICK);
        }
        keys_to_process.sort_unstable_by_key(|pos| nearest_player_distance_sq(*pos, &centers));

        for pos in keys_to_process {
            let Some(tickets) = chunk_tickets.add_tickets.swap_remove(&pos) else {
                continue;
            };

            let Some(chunk_entity) = chunk_index.get(pos) else {
                trace::mark(pos.into(), ColumnStage::Spawned);
                let chunk_entity = commands
                    .spawn((
                        SectionBundle::new(InDimension(dim), pos),
                        SectionTicketHolder(tickets),
                    ))
                    .id();
                chunk_index.insert(pos, chunk_entity);
                stages.spawned(chunk_entity, pos, dim, SectionStage::Loading);
                continue;
            };

            let Ok((mut ticket_holder, has_blocks)) = holders.get_mut(chunk_entity) else {
                continue;
            };
            ticket_holder.add_all(tickets);
            // A section on its way out still holds the blocks it was generated with, so a
            // ticket arriving before it goes takes it back rather than waiting for the
            // despawn and generating the same terrain again. One condemned before its blocks
            // arrived has nothing to keep and is generated again.
            if stages.get(chunk_entity) == Some(SectionStage::Unloading) {
                let back = if has_blocks {
                    SectionStage::Loaded
                } else {
                    SectionStage::Loading
                };
                stages.set(chunk_entity, back);
            }
        }
    }
}

fn remove_tickets_from_chunks(
    mut dims: Query<(&mut ChunkTicketsCommands, &SectionIndex)>,
    mut holders: Query<&mut SectionTicketHolder>,
    mut stages: SectionStages,
) {
    for (mut chunk_tickets, chunk_index) in dims.iter_mut() {
        for (pos, ticket_kinds) in chunk_tickets.remove_tickets.drain() {
            let Some(chunk_entity) = chunk_index.get(pos) else {
                continue;
            };
            let Ok(mut ticket_holder) = holders.get_mut(chunk_entity) else {
                continue;
            };
            for kind in ticket_kinds {
                ticket_holder.remove(kind);
            }
            if ticket_holder.0.is_empty() {
                stages.set(chunk_entity, SectionStage::Unloading);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;

    fn app_with_dimension() -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<SectionStageChanged>();
        app.add_systems(
            FixedUpdate,
            (
                despawn_chunks.before(ChunkSpawnSet),
                spawn_chunks.in_set(ChunkSpawnSet),
            ),
        );
        let dim = app
            .world_mut()
            .spawn((ChunkTicketsCommands::default(), SectionIndex::new()))
            .id();
        (app, dim)
    }

    fn condemned_section(app: &mut App, dim: Entity, pos: SectionPos, blocks: bool) -> Entity {
        let mut section = app.world_mut().spawn((
            SectionBundle::new(InDimension(dim), pos),
            SectionTicketHolder(Vec::new()),
        ));
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

    fn raise_ticket(app: &mut App, dim: Entity, pos: SectionPos) {
        app.world_mut()
            .get_mut::<ChunkTicketsCommands>(dim)
            .expect("ticket commands")
            .add_ticket(pos, Ticket::new(TicketKind::Forced));
    }

    #[test]
    fn a_cancelled_request_leaves_the_queue_rather_than_emptying_in_place() {
        let mut tickets = ChunkTicketsCommands::default();
        let pos = SectionPos::new(0, 0, 0);
        tickets.add_ticket(pos, Ticket::new(TicketKind::Forced));
        assert_eq!(tickets.queued_tickets(pos).len(), 1);

        tickets.remove_ticket(pos, TicketKind::Forced);

        assert!(
            tickets.add_tickets.is_empty(),
            "the request is gone, and nothing was left for the spawn to honour"
        );
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

        let index = app.world().get::<SectionIndex>(dim).expect("section index");
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
    /// The section still holds its blocks, so taking the ticket back is a great deal cheaper
    /// than despawning it and generating the same terrain again.
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
            app.world()
                .get::<SectionIndex>(dim)
                .expect("section index")
                .get(pos),
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
        assert_eq!(
            app.world()
                .get::<SectionTicketHolder>(dying)
                .expect("ticket holder")
                .0
                .len(),
            1,
            "the ticket that saved it is the one it now holds"
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
        let respawned = app
            .world()
            .get::<SectionIndex>(dim)
            .expect("section index")
            .get(pos)
            .expect("the ticket spawns a fresh section once the old one is gone");
        assert_ne!(respawned, dying);
    }
}
