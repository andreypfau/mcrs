use mcrs_voxel::{LocalPos, SectionPos};
use std::sync::Arc;

use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::{ColumnHeights, VoxelId};
use mcrs_minecraft_simulation::PendingTick;
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};

pub type SectionData = (BlockPalette, BiomePalette);
use crate::task::ColumnSource;
use mcrs_minecraft_heightmap::ColumnHeightmapSet;

#[derive(Debug, Clone)]
pub struct TerrainHeightmaps {
    pub surface: ColumnHeights,
    pub solid: ColumnHeights,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Filling,
    Filled,
    Running(u8),
    Ran(u8),
    Merging(u8),
    Merged(u8),
    Delivered,
}

impl Stage {
    pub fn in_flight(self) -> bool {
        matches!(self, Stage::Filling | Stage::Running(_) | Stage::Merging(_))
    }

    fn ordinal(self) -> u32 {
        let rung = |rung: u8, phase: u32| 2 + u32::from(rung) * 4 + phase;
        match self {
            Stage::Filling => 0,
            Stage::Filled => 1,
            Stage::Running(at) => rung(at, 0),
            Stage::Ran(at) => rung(at, 1),
            Stage::Merging(at) => rung(at, 2),
            Stage::Merged(at) => rung(at, 3),
            Stage::Delivered => u32::MAX,
        }
    }
}

impl Ord for Stage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

impl PartialOrd for Stage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub const fn rank(col: ColumnPos) -> u8 {
    (col.x.rem_euclid(3) * 3 + col.z.rem_euclid(3)) as u8
}

pub const fn cell_index(slot: usize, local: LocalPos) -> u32 {
    (slot * SectionPos::VOLUME + local.index()) as u32
}

pub struct ColumnDelta {
    pub source_rank: u8,
    pub writes: Vec<(u32, VoxelId)>,
    pub block_entities: Vec<GeneratedBlockEntity>,
    pub entities: Vec<GeneratedEntity>,
    pub ticks: Vec<PendingTick<GenerationKinds>>,
}

#[derive(Clone)]
pub struct FilledSnapshot {
    pub col: ColumnPos,
    pub y_sections: Arc<[i32]>,
    pub sections: Vec<Option<SectionData>>,
    pub terrain: Option<TerrainHeightmaps>,
    pub maps: Option<ColumnHeightmapSet>,
    pub source: ColumnSource,
    pub block_entities: Vec<GeneratedBlockEntity>,
    pub entities: Vec<GeneratedEntity>,
    pub ticks: Vec<PendingTick<GenerationKinds>>,
}

impl FilledSnapshot {
    #[inline]
    pub fn slot(&self, world_y: i32) -> Option<usize> {
        let first = *self.y_sections.first()?;
        usize::try_from((world_y >> 4) - first)
            .ok()
            .filter(|slot| *slot < self.y_sections.len())
    }
}

pub type RegionSnapshots = [Arc<FilledSnapshot>; 9];

#[inline]
pub fn region_slot(center: ColumnPos, col: ColumnPos) -> Option<usize> {
    let dx = col.x - center.x;
    let dz = col.z - center.z;
    if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dz) {
        return None;
    }
    Some(((dz + 1) * 3 + (dx + 1)) as usize)
}

#[inline]
pub fn region_column(center: ColumnPos, slot: usize) -> ColumnPos {
    ColumnPos::new(
        center.x + (slot % 3) as i32 - 1,
        center.z + (slot / 3) as i32 - 1,
    )
}

#[derive(Default)]
struct ColumnEntry {
    stage: Option<Stage>,
    filled: Option<Arc<FilledSnapshot>>,
    staged: Vec<Option<Arc<FilledSnapshot>>>,
    out: Vec<[Option<Arc<ColumnDelta>>; 9]>,
    block_entities_taken: bool,
    entities_taken: bool,
}

impl ColumnEntry {
    fn slot_mut<T: Default>(row: &mut Vec<T>, index: usize) -> &mut T {
        if row.len() <= index {
            row.resize_with(index + 1, T::default);
        }
        &mut row[index]
    }

    fn top(&self) -> Option<&Arc<FilledSnapshot>> {
        self.staged
            .iter()
            .rev()
            .flatten()
            .next()
            .or(self.filled.as_ref())
    }
}

#[derive(Default)]
pub struct StagingStore {
    columns: FxHashMap<ColumnPos, ColumnEntry>,
}

impl StagingStore {
    #[inline]
    pub fn stage(&self, col: ColumnPos) -> Option<Stage> {
        self.columns.get(&col)?.stage
    }

    #[inline]
    pub fn set_stage(&mut self, col: ColumnPos, stage: Stage) {
        self.columns.entry(col).or_default().stage = Some(stage);
    }

    pub fn forget(&mut self, col: ColumnPos) {
        self.columns.remove(&col);
    }

    pub fn insert_filled(&mut self, snapshot: FilledSnapshot) {
        let col = snapshot.col;
        self.columns.entry(col).or_default().filled = Some(Arc::new(snapshot));
    }

    pub fn filled(&self, col: ColumnPos) -> Option<&Arc<FilledSnapshot>> {
        self.columns.get(&col)?.filled.as_ref()
    }

    pub fn insert_saved(&mut self, col: ColumnPos, rungs: usize) {
        let entry = self.columns.entry(col).or_default();
        let Some(filled) = entry.filled.clone() else {
            return;
        };
        entry.staged = vec![Some(filled); rungs];
    }

    pub fn insert_staged(&mut self, col: ColumnPos, rung: usize, staged: Arc<FilledSnapshot>) {
        let entry = self.columns.entry(col).or_default();
        *ColumnEntry::slot_mut(&mut entry.staged, rung) = Some(staged);
    }

    pub fn base(&self, col: ColumnPos, rung: usize) -> Option<&Arc<FilledSnapshot>> {
        let entry = self.columns.get(&col)?;
        match rung.checked_sub(1) {
            Some(below) => entry.staged.get(below)?.as_ref(),
            None => entry.filled.as_ref(),
        }
    }

    pub fn merged(&self, col: ColumnPos) -> Option<&Arc<FilledSnapshot>> {
        self.columns.get(&col).and_then(ColumnEntry::top)
    }

    pub(crate) fn take_merged(&mut self, col: ColumnPos) -> Option<Arc<FilledSnapshot>> {
        let ColumnEntry { filled, staged, .. } = self.columns.remove(&col)?;
        staged.into_iter().rev().flatten().next().or(filled)
    }

    pub(crate) fn release_merged_below(&mut self, col: ColumnPos, rung: usize) {
        let Some(entry) = self.columns.get_mut(&col) else {
            return;
        };
        for row in entry.out.iter_mut().take(rung.saturating_add(1)) {
            *row = Default::default();
        }
        if entry.staged.get(rung).is_none_or(Option::is_none) {
            return;
        }
        entry.filled = None;
        for staged in entry.staged.iter_mut().take(rung) {
            *staged = None;
        }
    }

    pub fn take_block_entities(&mut self, col: ColumnPos) -> Vec<GeneratedBlockEntity> {
        let Some(entry) = self.columns.get_mut(&col) else {
            return Vec::new();
        };
        if std::mem::replace(&mut entry.block_entities_taken, true) {
            return Vec::new();
        }
        entry
            .top()
            .map(|snapshot| snapshot.block_entities.clone())
            .unwrap_or_default()
    }

    pub fn take_entities(&mut self, col: ColumnPos) -> Vec<GeneratedEntity> {
        let Some(entry) = self.columns.get_mut(&col) else {
            return Vec::new();
        };
        if std::mem::replace(&mut entry.entities_taken, true) {
            return Vec::new();
        }
        entry
            .top()
            .map(|snapshot| snapshot.entities.clone())
            .unwrap_or_default()
    }

    pub fn push_delta(
        &mut self,
        source: ColumnPos,
        rung: usize,
        target: ColumnPos,
        delta: ColumnDelta,
    ) {
        let Some(slot) = region_slot(source, target) else {
            debug_assert!(false, "{source:?} wrote into {target:?}, outside its 3×3");
            return;
        };
        let entry = self.columns.entry(source).or_default();
        ColumnEntry::slot_mut(&mut entry.out, rung)[slot] = Some(Arc::new(delta));
    }

    pub fn deltas(&self, col: ColumnPos, rung: usize) -> Vec<Arc<ColumnDelta>> {
        let mut by_rank: [Option<Arc<ColumnDelta>>; 9] = Default::default();
        for slot in 0..9 {
            let source = region_column(col, slot);
            let Some(entry) = self.columns.get(&source) else {
                continue;
            };
            let Some(delta) = entry
                .out
                .get(rung)
                .and_then(|row| row[region_slot(source, col)?].clone())
            else {
                continue;
            };
            by_rank[rank(source) as usize] = Some(delta);
        }
        by_rank.into_iter().flatten().collect()
    }

    pub fn region(&self, col: ColumnPos, rung: usize) -> Option<RegionSnapshots> {
        let slots: Vec<_> = (0..9)
            .map(|slot| self.base(region_column(col, slot), rung).cloned())
            .collect::<Option<_>>()?;
        slots.try_into().ok()
    }

    pub fn neighbourhood_reached(&self, col: ColumnPos, stage: Stage) -> bool {
        (0..9).all(|slot| {
            self.stage(region_column(col, slot))
                .is_some_and(|at| at >= stage)
        })
    }

    pub fn retain(&mut self, keep: impl Fn(ColumnPos, Stage) -> bool) {
        self.columns
            .retain(|col, entry| entry.stage.is_some_and(|stage| keep(*col, stage)));
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_columns_colour_like_positive_ones() {
        for x in -6i32..6 {
            for z in -6i32..6 {
                assert_eq!(
                    rank(ColumnPos::new(x, z)),
                    rank(ColumnPos::new(x + 3, z + 3)),
                    "the rank must be periodic in three"
                );
            }
        }
        assert_eq!(rank(ColumnPos::new(-1, -1)), rank(ColumnPos::new(2, 2)));
        assert!((0..9).all(|slot| {
            let col = region_column(ColumnPos::new(0, 0), slot);
            region_slot(ColumnPos::new(0, 0), col) == Some(slot)
        }));
    }

    fn bare(col: ColumnPos) -> FilledSnapshot {
        FilledSnapshot {
            col,
            y_sections: Arc::from(vec![0i32]),
            sections: vec![None],
            terrain: None,
            maps: None,
            source: ColumnSource::Generated,
            block_entities: Vec::new(),
            entities: Vec::new(),
            ticks: Vec::new(),
        }
    }

    #[test]
    fn a_window_is_only_offered_once_all_nine_are_filled() {
        let mut store = StagingStore::default();
        let centre = ColumnPos::new(4, -2);
        for slot in 0..9 {
            assert!(store.region(centre, 0).is_none());
            let col = region_column(centre, slot);
            store.insert_filled(bare(col));
            store.set_stage(col, Stage::Filled);
        }
        let region = store.region(centre, 0).expect("all nine are in the store");
        assert_eq!(region[4].col, centre);
        assert!(store.neighbourhood_reached(centre, Stage::Filled));
        assert!(!store.neighbourhood_reached(centre, Stage::Ran(0)));

        store.retain(|col, _| col == centre);
        assert!(store.region(centre, 0).is_none());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn a_rung_reads_the_merge_of_the_rung_below_it() {
        let mut store = StagingStore::default();
        let centre = ColumnPos::new(0, 0);
        for slot in 0..9 {
            let col = region_column(centre, slot);
            store.insert_filled(bare(col));
            store.set_stage(col, Stage::Filled);
        }
        assert!(store.region(centre, 0).is_some());
        assert!(
            store.region(centre, 1).is_none(),
            "nobody has merged the rung below yet"
        );

        for slot in 0..9 {
            let col = region_column(centre, slot);
            let mut merged = bare(col);
            merged.sections = vec![None, None];
            store.insert_staged(col, 0, Arc::new(merged));
            store.set_stage(col, Stage::Merged(0));
        }
        let region = store.region(centre, 1).expect("the rung below is merged");
        assert!(
            region.iter().all(|snapshot| snapshot.sections.len() == 2),
            "the second rung reads what the first one left, not the fill"
        );
        assert_eq!(
            store.region(centre, 0).expect("the fill is still there")[4]
                .sections
                .len(),
            1,
            "the fill stays put under the rungs above it"
        );
    }

    #[test]
    fn dropping_a_column_leaves_the_deltas_written_into_it_with_their_writers() {
        let mut store = StagingStore::default();
        let target = ColumnPos::new(0, 0);
        for slot in 0..9 {
            let source = region_column(target, slot);
            store.set_stage(source, Stage::Ran(0));
            store.push_delta(
                source,
                0,
                target,
                ColumnDelta {
                    source_rank: rank(source),
                    writes: vec![(cell_index(0, LocalPos::new(1, 2, 3)), VoxelId(7))],
                    block_entities: Vec::new(),
                    entities: Vec::new(),
                    ticks: Vec::new(),
                },
            );
        }
        assert_eq!(store.deltas(target, 0).len(), 9, "one delta per source");
        assert!(
            store.deltas(target, 1).is_empty(),
            "a rung nobody has run writes nothing"
        );

        store.push_delta(
            target,
            0,
            target,
            ColumnDelta {
                source_rank: rank(target),
                writes: vec![(cell_index(0, LocalPos::new(1, 2, 3)), VoxelId(9))],
                block_entities: Vec::new(),
                entities: Vec::new(),
                ticks: Vec::new(),
            },
        );
        assert_eq!(
            store.deltas(target, 0).len(),
            9,
            "a re-run replaces its delta"
        );

        store.forget(target);
        assert_eq!(store.stage(target), None);
        assert_eq!(
            store.deltas(target, 0).len(),
            8,
            "only the dropped column's own delta went with it"
        );
        for slot in 0..9 {
            let source = region_column(target, slot);
            if source == target {
                continue;
            }
            assert_eq!(
                store.stage(source),
                Some(Stage::Ran(0)),
                "{source:?} still holds what it wrote and owes nothing again"
            );
        }
    }

    #[test]
    fn a_second_delivery_of_one_column_drops_nothing_twice() {
        let col = ColumnPos::new(3, -4);
        let mut store = StagingStore::default();
        let mut merged = bare(col);
        merged.block_entities = vec![GeneratedBlockEntity::Beehive {
            x: 50,
            y: 70,
            z: -60,
            bees: Vec::new(),
        }];
        store.insert_staged(col, 0, Arc::new(merged));

        assert_eq!(store.take_block_entities(col).len(), 1);
        assert!(
            store.take_block_entities(col).is_empty(),
            "the second delivery of the same column has nothing left to spawn"
        );
    }

    #[test]
    fn eviction_drops_a_saved_column_s_block_entities() {
        let mut store = StagingStore::default();
        let col = ColumnPos::new(100, 0);
        let mut merged = bare(col);
        merged.source = ColumnSource::Saved;
        merged.block_entities = vec![GeneratedBlockEntity::Beehive {
            x: 1600,
            y: 70,
            z: 4,
            bees: Vec::new(),
        }];
        store.insert_staged(col, 0, Arc::new(merged));
        store.set_stage(col, Stage::Merged(0));

        store.retain(|_, _| false);

        assert!(
            store.take_block_entities(col).is_empty(),
            "eviction left a halo column's saved block entities behind"
        );
    }
}
