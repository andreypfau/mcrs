use mcrs_voxel_math::{LocalPos, SectionPos};
use std::sync::Arc;

use mcrs_minecraft_decoration::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_voxel_storage::VoxelId;
use rustc_hash::FxHashMap;

use crate::world::chunk::ColumnSource;
use crate::world::format::anvil::SectionData;
use crate::world::heightmap::{ColumnHeightmapSet, PreCarveHeightmaps};

/// How far a column has climbed. The order is the ladder's order, so a
/// readiness test is a comparison.
///
/// The four run-and-merge rungs repeat, once per rung of the dimension's
/// feature program, which is why the order cannot be the derived one: it goes
/// by rung first and by phase within it, while a derive would put every
/// `Running` before every `Ran`.
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

/// The colour index of the nine-colour scattering grid: where two units write
/// the same position, the write of the higher rank stands.
///
/// Euclidean remainders, so a column at `-1` colours like one at `2`.
pub const fn rank(col: ColumnPos) -> u8 {
    (col.x.rem_euclid(3) * 3 + col.z.rem_euclid(3)) as u8
}

/// A cell of a column, in the packed order the sections themselves use:
/// section slot, then y, then z, then x. Every column of a dimension shares one
/// section list, so the index means the same block in the source column and in
/// the target.
pub const fn cell_index(slot: usize, local: LocalPos) -> u32 {
    (slot * SectionPos::VOLUME + local.index()) as u32
}

/// What one unit wrote into one column that is not its own.
pub struct ColumnDelta {
    pub source_rank: u8,
    pub writes: Vec<(u32, VoxelId)>,
    /// The block entities among those writes, delivered with the column that
    /// holds them.
    pub block_entities: Vec<GeneratedBlockEntity>,
}

/// A column at rest: filled, which the first rung of every run in its 3×3 reads
/// and its own merge starts from; the merge of one rung, which the next rung
/// reads the same way; or the merge of the last, which delivery hands to the
/// ECS.
#[derive(Clone)]
pub struct FilledSnapshot {
    pub col: ColumnPos,
    pub y_sections: Arc<[i32]>,
    pub sections: Vec<Option<SectionData>>,
    /// `WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG`, frozen before the carvers ran.
    pub pre_carve: Option<PreCarveHeightmaps>,
    pub maps: Option<ColumnHeightmapSet>,
    pub source: ColumnSource,
    /// What the save held, or what the merged deltas brought.
    pub block_entities: Vec<GeneratedBlockEntity>,
}

impl FilledSnapshot {
    /// Which slot of the section list holds `world_y`, if the column covers it.
    #[inline]
    pub fn slot(&self, world_y: i32) -> Option<usize> {
        let first = *self.y_sections.first()?;
        usize::try_from((world_y >> 4) - first)
            .ok()
            .filter(|slot| *slot < self.y_sections.len())
    }
}

/// The nine snapshots one run reads, centre at index 4.
pub type RegionSnapshots = [Arc<FilledSnapshot>; 9];

/// Offset of a column of the 3×3 from its centre, or `None` outside it.
#[inline]
pub fn region_slot(center: ColumnPos, col: ColumnPos) -> Option<usize> {
    let dx = col.x - center.x;
    let dz = col.z - center.z;
    if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dz) {
        return None;
    }
    Some(((dz + 1) * 3 + (dx + 1)) as usize)
}

/// The column at region slot `slot` around `center`.
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
    /// The column as the merge of rung `k` left it. A rung nothing wrote into
    /// carries the snapshot of the one before it, so a column that grows
    /// nothing on a rung costs one pointer rather than a second copy of its
    /// palettes.
    staged: Vec<Option<Arc<FilledSnapshot>>>,
    /// What this column's run of rung `k` wrote into the column at slot `s` of
    /// its own 3×3.
    ///
    /// Held by the writer rather than by the target, which is what makes an
    /// eviction cheap: dropping a column loses nothing its neighbours owe it,
    /// so a column that comes back climbs its own ladder again and merges from
    /// the very same deltas. Nobody has to be sent back down.
    out: Vec<[Option<Arc<ColumnDelta>>; 9]>,
    entities_taken: bool,
}

impl ColumnEntry {
    fn slot_mut<T: Default>(row: &mut Vec<T>, index: usize) -> &mut T {
        if row.len() <= index {
            row.resize_with(index + 1, T::default);
        }
        &mut row[index]
    }

    /// The highest rung this column has merged, or the fill under a ladder with
    /// no rungs at all.
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

    /// A column the save already holds arrives decorated, so it stands at the
    /// top of the ladder with the same blocks on every rung and writes into
    /// nobody. Its neighbours read it and merge nothing from it, which is what
    /// having been decorated before it was written means.
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

    /// What a run of `rung` reads of `col`: the merge of the rung before it, or
    /// the filled column at the foot of the ladder.
    pub fn base(&self, col: ColumnPos, rung: usize) -> Option<&Arc<FilledSnapshot>> {
        let entry = self.columns.get(&col)?;
        match rung.checked_sub(1) {
            Some(below) => entry.staged.get(below)?.as_ref(),
            None => entry.filled.as_ref(),
        }
    }

    /// The column as the last rung left it, which is what a delivery hands to
    /// the ECS.
    ///
    /// Kept past the delivery, not consumed by it: sections ticketed after a
    /// column was delivered are delivered from the same blocks rather than from
    /// the undecorated snapshot.
    pub fn merged(&self, col: ColumnPos) -> Option<&Arc<FilledSnapshot>> {
        self.columns.get(&col).and_then(ColumnEntry::top)
    }

    /// The column's block entities, handed over on the first delivery so a
    /// column delivered again for a late section spawns nothing twice.
    pub fn take_block_entities(&mut self, col: ColumnPos) -> Vec<GeneratedBlockEntity> {
        let Some(entry) = self.columns.get_mut(&col) else {
            return Vec::new();
        };
        if std::mem::replace(&mut entry.entities_taken, true) {
            return Vec::new();
        }
        entry
            .top()
            .map(|snapshot| snapshot.block_entities.clone())
            .unwrap_or_default()
    }

    /// One delta per target of one run, replaced and not appended, so a column
    /// that climbs the same rung twice supersedes what it wrote the first time.
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

    /// What a merge of `col` at `rung` applies, in ascending source rank.
    ///
    /// The nine columns of a 3×3 cover the nine ranks exactly once, so ordering
    /// by rank is a fixed permutation of the neighbourhood and the last write
    /// standing is a function of position alone.
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

    /// The nine snapshots a run of `rung` over `col` reads, or `None` while any
    /// is missing.
    pub fn region(&self, col: ColumnPos, rung: usize) -> Option<RegionSnapshots> {
        let slots: Vec<_> = (0..9)
            .map(|slot| self.base(region_column(col, slot), rung).cloned())
            .collect::<Option<_>>()?;
        slots.try_into().ok()
    }

    /// Whether every column of the 3×3 around `col` has reached `stage`.
    pub fn neighbourhood_reached(&self, col: ColumnPos, stage: Stage) -> bool {
        (0..9).all(|slot| {
            self.stage(region_column(col, slot))
                .is_some_and(|at| at >= stage)
        })
    }

    /// Drop everything no longer within reach of a column somebody wants.
    ///
    /// Refilling an evicted column is bit-identical and costs one fill, so the
    /// bound is the halo and nothing else. Nothing else has to be undone
    /// either: a delta lives with the column that wrote it, so a target that
    /// falls out of the halo and comes back merges from deltas that never went
    /// anywhere, and a merge that names a source the halo has dropped simply
    /// waits for that source to climb again.
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
            pre_carve: None,
            maps: None,
            source: ColumnSource::Generated,
            block_entities: Vec::new(),
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

    /// A rung reads the merge of the one below it, and the foot of the ladder
    /// is the filled column. A window is only offered once every one of the
    /// nine has climbed that far.
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

    /// A delta lives with the column that wrote it, so dropping the column it
    /// was written into costs nothing but the fill: the writers stay where they
    /// are, and a rebuilt target merges from the very same deltas.
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

    /// A column stays merged past its delivery, so a section ticketed later is
    /// delivered from the same blocks — and would carry the same block entities
    /// a second time if the delivery did not take them.
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
