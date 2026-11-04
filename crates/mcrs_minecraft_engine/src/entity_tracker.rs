//! Which subjects each viewer has on its client: one grid of sections and one radius.

use crate::entity_motion::DeltaBase;
use bevy_ecs::entity::{Entity, EntityHashMap, EntityHashSet};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_protocol::VarInt;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

/// The distance in sections, on each axis, inside which a subject is in the visible set of a
/// viewer. Five sections are 80 blocks.
pub const TRACK_RADIUS_SECTIONS: i32 = 5;

/// The cell of the grid that holds a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackCell {
    pub dimension: Entity,
    pub section: SectionPos,
}

impl TrackCell {
    /// Whether a viewer in this cell sees a subject in `other`. The relation is symmetric.
    #[must_use]
    pub fn sees(self, other: Self) -> bool {
        self.dimension == other.dimension
            && (self.section.0 - other.section.0).abs().max_element() <= TRACK_RADIUS_SECTIONS
    }
}

/// A viewer and a subject that it gains or loses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewPair {
    pub viewer: Entity,
    pub subject: Entity,
}

/// A player that the tracker holds. Each player is a viewer and a subject, so its visible set is
/// also the set of the viewers that have it.
#[derive(Debug)]
pub struct TrackedSubject {
    pub cell: TrackCell,
    pub session: PlayerSession,
    pub wire_id: VarInt,
    pub base: DeltaBase,
    /// The tracker tick of the last absolute position that the viewers received.
    pub keyframe_tick: u64,
    visible: EntityHashSet,
}

impl TrackedSubject {
    pub fn new(cell: TrackCell, session: PlayerSession, wire_id: VarInt, base: DeltaBase) -> Self {
        Self {
            cell,
            session,
            wire_id,
            base,
            keyframe_tick: 0,
            visible: EntityHashSet::default(),
        }
    }
}

/// The visible sets of the players of a world, derived from their positions. Only the tracker
/// system and the removal of a player change it.
///
/// The work of a tick is the subjects that change cell times the cells and subjects in their
/// radius, so it does not grow with the view distance or with all subjects times all viewers.
#[derive(Resource, Default, Debug)]
pub struct EntityTracker {
    cells: FxHashMap<TrackCell, EntityHashSet>,
    subjects: EntityHashMap<TrackedSubject>,
    /// The pairs that entered since the last flush, net of pairs that left again.
    entered: BTreeSet<ViewPair>,
    /// The pairs that left since the last flush, with the wire id of the subject, which can be
    /// gone by the flush.
    left: BTreeMap<ViewPair, VarInt>,
    tick: u64,
    generation: u64,
}

impl EntityTracker {
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&TrackedSubject> {
        self.subjects.get(&entity)
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut TrackedSubject> {
        self.subjects.get_mut(&entity)
    }

    /// The subjects that `viewer` has on its client, which are also the viewers that have it.
    pub fn visible(&self, viewer: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.subjects
            .get(&viewer)
            .into_iter()
            .flat_map(|subject| subject.visible.iter().copied())
    }

    /// The sessions of the viewers that have `subject` on their client.
    #[must_use]
    pub fn viewer_sessions(&self, subject: Entity) -> SmallVec<[PlayerSession; 8]> {
        self.visible(subject)
            .filter_map(|viewer| self.subjects.get(&viewer).map(|viewer| viewer.session))
            .collect()
    }

    /// Whether `viewer` has `subject` on its client.
    #[must_use]
    pub fn sees(&self, viewer: Entity, subject: Entity) -> bool {
        self.subjects
            .get(&viewer)
            .is_some_and(|tracked| tracked.visible.contains(&subject))
    }

    /// Every subject, in no fixed order.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &TrackedSubject)> {
        self.subjects
            .iter()
            .map(|(&entity, subject)| (entity, subject))
    }

    /// Changes each time a subject comes, goes, or moves to another cell.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subjects.is_empty()
    }

    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn advance_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Adds `entity` in the cell of `subject` and pairs it with every subject in its radius.
    pub fn insert(&mut self, entity: Entity, subject: TrackedSubject) {
        let cell = subject.cell;
        self.generation = self.generation.wrapping_add(1);
        self.cells.entry(cell).or_default().insert(entity);
        self.subjects.insert(entity, subject);
        self.gain(entity, cell, None);
    }

    /// Moves `entity` to `cell`, and pairs and unpairs it with the subjects that come into and
    /// go out of its radius.
    pub fn relocate(&mut self, entity: Entity, cell: TrackCell) {
        let Some(subject) = self.subjects.get_mut(&entity) else {
            return;
        };
        let old = mem::replace(&mut subject.cell, cell);
        if old == cell {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.leave_cell(entity, old);
        self.cells.entry(cell).or_default().insert(entity);
        let lost: Vec<Entity> = self
            .visible(entity)
            .filter(|other| {
                self.subjects
                    .get(other)
                    .is_none_or(|other| !cell.sees(other.cell))
            })
            .collect();
        for other in lost {
            self.unpair(entity, other);
        }
        self.gain(entity, cell, Some(old));
    }

    /// Removes `entity`; each viewer that had it loses it at the next flush.
    pub fn forget(&mut self, entity: Entity) {
        let Some(subject) = self.subjects.remove(&entity) else {
            return;
        };
        self.generation = self.generation.wrapping_add(1);
        self.leave_cell(entity, subject.cell);
        for other in subject.visible {
            if let Some(viewer) = self.subjects.get_mut(&other) {
                viewer.visible.remove(&entity);
            }
            self.record_left(
                ViewPair {
                    viewer: other,
                    subject: entity,
                },
                subject.wire_id,
            );
        }
        self.entered
            .retain(|pair| pair.viewer != entity && pair.subject != entity);
        self.left.retain(|pair, _| pair.viewer != entity);
    }

    /// The pairs that left since the last call, sorted by viewer.
    pub fn take_left(&mut self) -> BTreeMap<ViewPair, VarInt> {
        mem::take(&mut self.left)
    }

    /// The pairs that entered since the last call, sorted by viewer.
    pub fn take_entered(&mut self) -> BTreeSet<ViewPair> {
        mem::take(&mut self.entered)
    }

    fn leave_cell(&mut self, entity: Entity, cell: TrackCell) {
        if let Some(members) = self.cells.get_mut(&cell) {
            members.remove(&entity);
            if members.is_empty() {
                self.cells.remove(&cell);
            }
        }
    }

    /// Pairs `entity`, now in `cell`, with the subjects of the cells in its radius that were out
    /// of the radius of `old`. A subject in such a cell was not visible before.
    fn gain(&mut self, entity: Entity, cell: TrackCell, old: Option<TrackCell>) {
        let mut gained: Vec<Entity> = Vec::new();
        let radius = TRACK_RADIUS_SECTIONS;
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    let near = TrackCell {
                        dimension: cell.dimension,
                        section: SectionPos::new(
                            cell.section.x + dx,
                            cell.section.y + dy,
                            cell.section.z + dz,
                        ),
                    };
                    if old.is_some_and(|old| old.sees(near)) {
                        continue;
                    }
                    if let Some(members) = self.cells.get(&near) {
                        gained.extend(members.iter().filter(|&&other| other != entity));
                    }
                }
            }
        }
        gained.sort_unstable();
        for other in gained {
            self.pair(entity, other);
        }
    }

    fn pair(&mut self, first: Entity, second: Entity) {
        let paired = self
            .subjects
            .get_mut(&first)
            .is_some_and(|subject| subject.visible.insert(second));
        if !paired {
            return;
        }
        if let Some(subject) = self.subjects.get_mut(&second) {
            subject.visible.insert(first);
        }
        for pair in pair_views(first, second) {
            if self.left.remove(&pair).is_none() {
                self.entered.insert(pair);
            }
        }
    }

    fn unpair(&mut self, first: Entity, second: Entity) {
        let (Some(first_id), Some(second_id)) = (
            self.drop_visible(first, second),
            self.drop_visible(second, first),
        ) else {
            return;
        };
        let [first_views_second, second_views_first] = pair_views(first, second);
        self.record_left(first_views_second, second_id);
        self.record_left(second_views_first, first_id);
    }

    /// Takes `subject` out of the visible set of `viewer`, and gives the wire id of `viewer`.
    fn drop_visible(&mut self, viewer: Entity, subject: Entity) -> Option<VarInt> {
        let viewer = self.subjects.get_mut(&viewer)?;
        viewer.visible.remove(&subject);
        Some(viewer.wire_id)
    }

    fn record_left(&mut self, pair: ViewPair, wire_id: VarInt) {
        if !self.entered.remove(&pair) {
            self.left.insert(pair, wire_id);
        }
    }
}

/// The two views of a pair of players: `first` sees `second`, and `second` sees `first`.
fn pair_views(first: Entity, second: Entity) -> [ViewPair; 2] {
    [
        ViewPair {
            viewer: first,
            subject: second,
        },
        ViewPair {
            viewer: second,
            subject: first,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::DVec3;
    use mcrs_minecraft_engine_world::entity::physics::Transform;

    fn cell(x: i32) -> TrackCell {
        TrackCell {
            dimension: Entity::from_raw_u32(1).unwrap(),
            section: SectionPos::new(x, 4, 0),
        }
    }

    fn subject(x: i32, session: u64) -> TrackedSubject {
        TrackedSubject::new(
            cell(x),
            PlayerSession(session),
            VarInt(session as i32),
            DeltaBase::new(&Transform::from_translation(DVec3::ZERO)),
        )
    }

    #[test]
    fn a_pair_that_leaves_and_comes_back_in_one_tick_sends_nothing() {
        let mut tracker = EntityTracker::default();
        let [a, b] = [10, 11].map(|index| Entity::from_raw_u32(index).unwrap());
        tracker.insert(a, subject(0, 1));
        tracker.insert(b, subject(2, 2));
        assert_eq!(tracker.take_entered().len(), 2, "both see each other");

        tracker.relocate(a, cell(-20));
        tracker.relocate(a, cell(0));

        assert!(tracker.take_entered().is_empty());
        assert!(tracker.take_left().is_empty());
        assert_eq!(tracker.visible(b).collect::<Vec<_>>(), [a]);
    }
}
