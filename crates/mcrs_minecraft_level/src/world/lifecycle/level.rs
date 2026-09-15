use bevy_ecs::prelude::Component;
use indexmap::IndexSet;
use mcrs_minecraft_core::SectionPos;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

pub const ENTITY_TICKING_LEVEL: u8 = 31;
pub const BLOCK_TICKING_LEVEL: u8 = 32;
pub const FULL_LEVEL: u8 = 33;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FullStatus {
    Inaccessible,
    Full,
    BlockTicking,
    EntityTicking,
}

impl FullStatus {
    pub fn from_level(level: u8) -> Self {
        match level {
            ..=ENTITY_TICKING_LEVEL => Self::EntityTicking,
            BLOCK_TICKING_LEVEL => Self::BlockTicking,
            FULL_LEVEL => Self::Full,
            _ => Self::Inaccessible,
        }
    }
}

/// A section's level is the lower of its own source and one more than the lowest of its 26
/// neighbours. A level at or past `absent` is not stored.
#[derive(Debug)]
pub struct LevelField {
    absent: u8,
    levels: FxHashMap<SectionPos, u8>,
    relight: Vec<Vec<SectionPos>>,
    unlight: Vec<(SectionPos, u8)>,
}

impl LevelField {
    pub fn new(absent: u8) -> Self {
        Self {
            absent,
            levels: FxHashMap::default(),
            relight: vec![Vec::new(); absent as usize],
            unlight: Vec::new(),
        }
    }

    pub fn level(&self, pos: SectionPos) -> u8 {
        self.levels.get(&pos).copied().unwrap_or(self.absent)
    }

    /// Brings the field in line with the sources at `changed`, recording into `before` the
    /// level every section it moved held when the update began.
    pub fn update(
        &mut self,
        changed: &[SectionPos],
        source: impl Fn(SectionPos) -> u8,
        before: &mut FxHashMap<SectionPos, u8>,
    ) {
        let absent = self.absent;
        for &pos in changed {
            let current = self.level(pos);
            if source(pos).min(absent) > current {
                self.set(pos, absent, before);
                self.unlight.push((pos, current));
            }
        }
        // Whatever leaned on a level that went away is cleared, and every neighbour that could
        // stand in for it is kept as a seed to fill the cleared region back in.
        while let Some((pos, old)) = self.unlight.pop() {
            for neighbour in neighbours(pos) {
                let level = self.level(neighbour);
                if level == absent {
                    continue;
                }
                if level == old + 1 && source(neighbour) > level {
                    self.set(neighbour, absent, before);
                    self.unlight.push((neighbour, level));
                } else if level <= old + 1 {
                    self.relight[level as usize].push(neighbour);
                }
            }
        }
        for &pos in changed {
            let level = source(pos);
            if level < absent && level < self.level(pos) {
                self.set(pos, level, before);
                self.relight[level as usize].push(pos);
            }
        }
        for level in 0..absent {
            let mut seeds = std::mem::take(&mut self.relight[level as usize]);
            let next = level + 1;
            if next < absent {
                for &pos in &seeds {
                    if self.level(pos) != level {
                        continue;
                    }
                    for neighbour in neighbours(pos) {
                        if next < self.level(neighbour) {
                            self.set(neighbour, next, before);
                            self.relight[next as usize].push(neighbour);
                        }
                    }
                }
            }
            seeds.clear();
            self.relight[level as usize] = seeds;
        }
    }

    fn set(&mut self, pos: SectionPos, level: u8, before: &mut FxHashMap<SectionPos, u8>) {
        let old = self.level(pos);
        if old == level {
            return;
        }
        before.entry(pos).or_insert(old);
        if level >= self.absent {
            self.levels.remove(&pos);
        } else {
            self.levels.insert(pos, level);
        }
    }
}

fn neighbours(pos: SectionPos) -> impl Iterator<Item = SectionPos> {
    (-1..=1)
        .flat_map(move |dy| {
            (-1..=1).flat_map(move |dz| {
                (-1..=1).map(move |dx| SectionPos::new(pos.x + dx, pos.y + dy, pos.z + dz))
            })
        })
        .filter(move |neighbour| *neighbour != pos)
}

/// The loading and simulation levels of a dimension's sections, derived from its
/// `SectionTickets` by `propagate_section_levels`, the only system that moves them.
#[derive(Component, Debug)]
pub struct SectionLevels {
    /// Past full the reference's levels stand for the generation statuses a chunk climbs. Here
    /// the staging store owns what generation depends on, so nothing past full is kept.
    pub(crate) loading: LevelField,
    pub(crate) simulation: LevelField,
    /// Became loaded, and `spawn_chunks` has not dealt with it yet.
    pub(crate) pending_spawn: IndexSet<SectionPos, FxBuildHasher>,
    /// Stopped being loaded, and `release_sections` has not dealt with it yet.
    pub(crate) pending_release: FxHashSet<SectionPos>,
}

impl Default for SectionLevels {
    fn default() -> Self {
        Self {
            loading: LevelField::new(FULL_LEVEL + 1),
            simulation: LevelField::new(FULL_LEVEL),
            pending_spawn: IndexSet::default(),
            pending_release: FxHashSet::default(),
        }
    }
}

impl SectionLevels {
    pub fn is_loaded(&self, pos: SectionPos) -> bool {
        self.loading.level(pos) <= FULL_LEVEL
    }

    pub fn status(&self, pos: SectionPos) -> FullStatus {
        FullStatus::from_level(self.loading.level(pos))
    }

    pub fn is_block_ticking(&self, pos: SectionPos) -> bool {
        self.simulation.level(pos) <= BLOCK_TICKING_LEVEL
    }

    pub fn is_entity_ticking(&self, pos: SectionPos) -> bool {
        self.simulation.level(pos) <= ENTITY_TICKING_LEVEL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chebyshev(a: SectionPos, b: SectionPos) -> i32 {
        (a.x - b.x)
            .abs()
            .max((a.y - b.y).abs())
            .max((a.z - b.z).abs())
    }

    #[test]
    fn a_level_reaches_one_ring_further_for_every_step_it_has_left_to_full() {
        let mut field = LevelField::new(FULL_LEVEL + 1);
        let origin = SectionPos::new(0, 0, 0);
        field.update(
            &[origin],
            |pos| {
                if pos == origin {
                    ENTITY_TICKING_LEVEL
                } else {
                    u8::MAX
                }
            },
            &mut FxHashMap::default(),
        );

        let status = |pos| FullStatus::from_level(field.level(pos));
        assert_eq!(status(origin), FullStatus::EntityTicking);
        assert_eq!(status(SectionPos::new(1, -1, 1)), FullStatus::BlockTicking);
        assert_eq!(status(SectionPos::new(-2, 0, 2)), FullStatus::Full);
        assert_eq!(status(SectionPos::new(3, 0, 0)), FullStatus::Inaccessible);
    }

    /// Adding, lowering, raising and removing sources in any order has to leave exactly the
    /// field a recompute from scratch gives.
    #[test]
    fn the_field_matches_a_recompute_from_scratch_after_every_change() {
        const ABSENT: u8 = 6;
        let mut field = LevelField::new(ABSENT);
        let mut sources: FxHashMap<SectionPos, u8> = FxHashMap::default();
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };

        for step in 0..150 {
            let mut changed = Vec::new();
            for _ in 0..1 + next(4) {
                let pos =
                    SectionPos::new(next(7) as i32 - 3, next(3) as i32 - 1, next(7) as i32 - 3);
                if next(3) == 0 {
                    sources.remove(&pos);
                } else {
                    sources.insert(pos, next(ABSENT as u64 + 2) as u8);
                }
                changed.push(pos);
            }
            field.update(
                &changed,
                |pos| sources.get(&pos).copied().unwrap_or(u8::MAX),
                &mut FxHashMap::default(),
            );

            for x in -9..=9 {
                for y in -7..=7 {
                    for z in -9..=9 {
                        let pos = SectionPos::new(x, y, z);
                        let expected = sources
                            .iter()
                            .map(|(source, level)| *level as i32 + chebyshev(pos, *source))
                            .min()
                            .map_or(ABSENT, |level| level.min(ABSENT as i32) as u8);
                        assert_eq!(field.level(pos), expected, "step {step} at {pos:?}");
                    }
                }
            }
        }
    }
}
