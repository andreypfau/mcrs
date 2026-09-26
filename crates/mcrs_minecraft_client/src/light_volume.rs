use std::ops::RangeInclusive;
use std::sync::{Arc, LazyLock};

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::light::block_light_registry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ColumnPos, SectionPos};
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::level::LightBounds;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::layout::{
    Emitter, PackedBrick, lanes, neighbours, pack, reaching_types,
};
use mcrs_minecraft_light_color::region::section_bricks;
use mcrs_minecraft_render::{CameraOrigin, RenderPath, ShownPath};
use mcrs_minecraft_render_deferred::{VolumeCommand, VolumeQueue};

use crate::columns::{
    AIR, BlockSource, ClientTerrainSet, Column, ColumnChange, ColumnStore, Extent, SECTION_VOLUME,
};

static BRICKS_PER_FRAME: LazyLock<usize> = LazyLock::new(crate::config::color_bricks);

static AIR_SECTION: [u16; SECTION_VOLUME] = [AIR; SECTION_VOLUME];

pub struct LightVolumePlugin;

impl Plugin for LightVolumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightVolumeFeed>().add_systems(
            Update,
            feed_light_volume
                .in_set(ClientTerrainSet::Build)
                .run_if(resource_exists::<ColumnStore>),
        );
    }
}

/// What the feeder has handed the renderer and what it still owes it, kept only while the
/// deferred path is shown. The column store stays the truth.
#[derive(Resource, Default)]
pub struct LightVolumeFeed {
    active: bool,
    generation: u64,
    emitters: HashMap<ColumnPos, Vec<i32>>,
    built: HashMap<IVec3, Vec<Emitter>>,
    to_build: HashSet<IVec3>,
    building: HashMap<IVec3, Task<PackedBrick>>,
    dirty: HashSet<IVec3>,
}

/// Bricks reach one column past the coloured radius, so every coloured section has all 27 of
/// its neighbours built.
#[derive(Clone, Copy, Debug)]
struct Scope {
    camera: IVec3,
    radius: i32,
    extent: Extent,
}

impl Scope {
    fn columns_from_camera(&self, column: ColumnPos) -> i32 {
        (column.x - self.camera.x)
            .abs()
            .max((column.z - self.camera.z).abs())
    }

    fn max_y(&self) -> i32 {
        self.extent.min_section_y + self.extent.sections as i32 - 1
    }

    fn brick_rows(&self) -> RangeInclusive<i32> {
        self.extent.min_section_y - 1..=self.max_y()
    }

    fn colours(&self, section: IVec3) -> bool {
        self.columns_from_camera(column_of(section)) <= self.radius
            && (self.extent.min_section_y..=self.max_y()).contains(&section.y)
    }
}

fn column_of(section: IVec3) -> ColumnPos {
    ColumnPos::new(section.x, section.z)
}

fn emitter_rows(column: &Column, registry: &LightRegistry) -> Vec<i32> {
    column
        .sections()
        .filter_map(|(y, section)| {
            section?
                .states
                .iter()
                .any(|&state| !registry.emission(VoxelId(state)).is_zero())
                .then_some(y)
        })
        .collect()
}

/// The rows of `column` whose brick is wanted: some emitter section lies within two sections of
/// them on every axis.
fn wanted_rows<'a>(
    scope: &Scope,
    column: ColumnPos,
    loaded: impl Fn(ColumnPos) -> bool,
    emitters: impl Fn(ColumnPos) -> Option<&'a [i32]>,
) -> Vec<i32> {
    if !loaded(column) || scope.columns_from_camera(column) > scope.radius + 1 {
        return Vec::new();
    }
    let mut near = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let around = ColumnPos::new(column.x + dx, column.z + dz);
            near.extend(emitters(around).into_iter().flatten().copied());
        }
    }
    scope
        .brick_rows()
        .filter(|y| near.iter().any(|emitter| (emitter - y).abs() <= 2))
        .collect()
}

/// A brick holds the east, up and south vetoes of its own cells, so a column's arrival changes
/// the bricks of its west and north neighbours as well as its own.
fn rebuilt_by(at: ColumnPos) -> [ColumnPos; 3] {
    [
        at,
        ColumnPos::new(at.x - 1, at.z),
        ColumnPos::new(at.x, at.z - 1),
    ]
}

fn dirtied_by(at: ColumnPos, extent: Extent) -> impl Iterator<Item = IVec3> {
    let rows = extent.min_section_y - 1..=extent.min_section_y + extent.sections as i32;
    rows.flat_map(move |y| {
        (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| IVec3::new(at.x + dx, y, at.z + dz)))
    })
}

/// A loaded column's empty section is air; only a missing column is unloaded.
fn cells<'a, S: BlockSource>(
    source: &'a S,
) -> impl Fn(SectionPos) -> Option<&'a [u16; SECTION_VOLUME]> + 'a {
    move |at| {
        let column = source.column(at.x, at.z)?;
        Some(
            column
                .section(at.y)
                .map_or(&AIR_SECTION, |section| &*section.blocks),
        )
    }
}

impl LightVolumeFeed {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn arrive(
        &mut self,
        at: ColumnPos,
        emitter_rows: Vec<i32>,
        scope: &Scope,
        loaded: impl Fn(ColumnPos) -> bool,
    ) {
        let lit = !emitter_rows.is_empty();
        if lit {
            self.emitters.insert(at, emitter_rows);
        } else {
            self.emitters.remove(&at);
        }
        let stale = rebuilt_by(at);
        for dz in -2..=2 {
            for dx in -2..=2 {
                let column = ColumnPos::new(at.x + dx, at.z + dz);
                let rebuilt = stale.contains(&column);
                if !rebuilt && !lit {
                    continue;
                }
                let rows = wanted_rows(scope, column, &loaded, |around| {
                    self.emitters.get(&around).map(Vec::as_slice)
                });
                for y in rows {
                    let section = IVec3::new(column.x, y, column.z);
                    if rebuilt || !self.built.contains_key(&section) {
                        self.to_build.insert(section);
                    }
                }
            }
        }
        for section in dirtied_by(at, scope.extent) {
            if scope.colours(section) && loaded(column_of(section)) {
                self.dirty.insert(section);
            }
        }
    }

    fn depart(&mut self, at: ColumnPos) {
        self.emitters.remove(&at);
        self.to_build.retain(|&section| column_of(section) != at);
        self.building.retain(|&section, _| column_of(section) != at);
    }

    fn observe(
        &mut self,
        change: &ColumnChange,
        store: &ColumnStore,
        scope: &Scope,
        rows: impl Fn(&Column) -> Vec<i32>,
    ) {
        match change {
            ColumnChange::Arrived(at, _) => {
                if let Some(column) = store.get(*at) {
                    self.arrive(*at, rows(column), scope, |c| store.holds(c));
                }
            }
            ColumnChange::Departed(at, _) => self.depart(*at),
            ColumnChange::Relit(..) => {}
        }
    }

    fn land(&mut self, queue: &VolumeQueue) {
        let built = &mut self.built;
        self.building
            .retain(|&section, task| match check_ready(task) {
                Some(brick) => {
                    queue.push(VolumeCommand::Brick {
                        section,
                        words: brick.words,
                    });
                    built.insert(section, brick.emitters);
                    false
                }
                None => true,
            });
    }

    fn admit(
        &mut self,
        scope: &Scope,
        store: &ColumnStore,
        registry: &Arc<LightRegistry>,
        colours: &LightColors,
        per_frame: usize,
    ) {
        let mut next: Vec<IVec3> = self
            .to_build
            .iter()
            .copied()
            .filter(|section| !self.building.contains_key(section))
            .collect();
        if next.len() > per_frame {
            next.select_nth_unstable_by_key(per_frame, |&section| {
                (section - scope.camera).length_squared()
            });
            next.truncate(per_frame);
        }
        let bounds = LightBounds::new(scope.extent.min_section_y, scope.max_y());
        let pool = AsyncComputeTaskPool::get();
        for section in next {
            self.to_build.remove(&section);
            let column = column_of(section);
            if !store.holds(column) {
                continue;
            }
            let world = store.around(column);
            let (registry, colours) = (registry.clone(), colours.clone());
            let task = pool.spawn(async move {
                let at = SectionPos(section);
                pack(&section_bricks(
                    at,
                    bounds,
                    &registry,
                    &colours,
                    cells(&world),
                ))
            });
            self.building.insert(section, task);
        }
    }

    /// Dirty sections none of whose 27 neighbours still has a brick to build.
    fn take_ready(&mut self) -> Vec<IVec3> {
        let pending = self.to_build.len() + self.building.len();
        let ready: Vec<IVec3> = if pending == 0 {
            self.dirty.iter().copied().collect()
        } else if pending * 27 < self.dirty.len() {
            let blocked: HashSet<IVec3> = self
                .to_build
                .iter()
                .chain(self.building.keys())
                .flat_map(|&section| neighbours(SectionPos(section)).map(|n| n.0))
                .collect();
            self.dirty
                .iter()
                .copied()
                .filter(|section| !blocked.contains(section))
                .collect()
        } else {
            self.dirty
                .iter()
                .copied()
                .filter(|&section| {
                    neighbours(SectionPos(section))
                        .all(|n| !self.to_build.contains(&n.0) && !self.building.contains_key(&n.0))
                })
                .collect()
        };
        for section in &ready {
            self.dirty.remove(section);
        }
        ready
    }

    fn push_dirty(&mut self, colours: &LightColors, queue: &VolumeQueue) {
        for section in self.take_ready() {
            let types = reaching_types(SectionPos(section), |n| {
                self.built.get(&n.0).map(Vec::as_slice)
            });
            queue.push(VolumeCommand::Dirty {
                section,
                lanes: lanes(&types, colours),
            });
        }
    }
}

fn feed_light_volume(
    mut feed: ResMut<LightVolumeFeed>,
    mut changes: MessageReader<ColumnChange>,
    store: Res<ColumnStore>,
    shown: Res<ShownPath>,
    queue: Res<VolumeQueue>,
    origin: Res<CameraOrigin>,
    colours: Option<Res<LightColors>>,
    blocks: Option<Res<Blocks>>,
) {
    let radius = queue.radius();
    let (Some(colours), Some(blocks), Some(extent), RenderPath::Deferred, 1..) =
        (colours, blocks, store.extent(), shown.get(), radius)
    else {
        if feed.active {
            feed.clear();
        }
        return;
    };
    let feed = &mut *feed;
    let registry = block_light_registry(&blocks);
    let rows = |column: &Column| emitter_rows(column, &registry);
    let scope = Scope {
        camera: origin.section,
        radius: radius as i32,
        extent,
    };
    let generation = queue.generation();
    if !feed.active || feed.generation != generation {
        changes.clear();
        feed.clear();
        feed.active = true;
        feed.generation = generation;
        queue.push(VolumeCommand::Reset {
            min_section_y: extent.min_section_y,
            sections: extent.sections as u32,
        });
        let mut resident: Vec<ColumnPos> = store.resident().map(|(at, _)| at).collect();
        resident.sort_unstable_by_key(|&at| scope.columns_from_camera(at));
        for at in resident {
            if let Some(column) = store.get(at) {
                feed.arrive(at, rows(column), &scope, |c| store.holds(c));
            }
        }
    } else {
        for change in changes.read() {
            feed.observe(change, &store, &scope, rows);
        }
    }
    feed.land(&queue);
    feed.admit(&scope, &store, &registry, &colours, *BRICKS_PER_FRAME);
    feed.push_dirty(&colours, &queue);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::{BIOME_CELLS, Section};

    const EXTENT: Extent = Extent {
        min_section_y: 0,
        sections: 4,
    };

    fn scope(radius: i32) -> Scope {
        Scope {
            camera: IVec3::ZERO,
            radius,
            extent: EXTENT,
        }
    }

    fn columns(sections: &HashSet<IVec3>) -> HashSet<ColumnPos> {
        sections.iter().map(|&section| column_of(section)).collect()
    }

    fn square(reach: i32) -> impl Iterator<Item = ColumnPos> {
        (-reach..=reach).flat_map(move |dz| (-reach..=reach).map(move |dx| ColumnPos::new(dx, dz)))
    }

    #[test]
    fn an_arrival_rebuilds_its_own_and_its_west_and_north_neighbours_bricks() {
        let scope = scope(6);
        let mut feed = LightVolumeFeed::default();
        for column in square(4) {
            feed.emitters.insert(column, vec![1]);
            for y in scope.brick_rows() {
                feed.built
                    .insert(IVec3::new(column.x, y, column.z), Vec::new());
            }
        }
        let at = ColumnPos::new(1, 1);
        feed.arrive(at, vec![1], &scope, |_| true);
        let rebuilt = [at, ColumnPos::new(0, 1), ColumnPos::new(1, 0)];
        assert_eq!(columns(&feed.to_build), HashSet::from(rebuilt));
        for column in rebuilt {
            let rows = feed
                .to_build
                .iter()
                .filter(|&&section| column_of(section) == column)
                .count();
            assert_eq!(rows, scope.brick_rows().count(), "{column:?}");
        }
    }

    #[test]
    fn an_arrival_builds_the_bricks_its_emitters_bring_into_reach() {
        let scope = scope(6);
        let mut feed = LightVolumeFeed::default();
        feed.arrive(ColumnPos::new(0, 0), vec![2], &scope, |_| true);
        assert_eq!(columns(&feed.to_build), square(2).collect());
        let rows: HashSet<i32> = feed.to_build.iter().map(|section| section.y).collect();
        assert_eq!(rows, HashSet::from([0, 1, 2, 3]));
    }

    #[test]
    fn an_arrival_dirties_every_section_within_one_of_its_own() {
        let scope = scope(8);
        let mut feed = LightVolumeFeed::default();
        let at = ColumnPos::new(-3, 5);
        let missing = ColumnPos::new(-2, 6);
        feed.arrive(at, Vec::new(), &scope, |column| column != missing);
        let expected: HashSet<IVec3> = square(1)
            .map(|d| ColumnPos::new(at.x + d.x, at.z + d.z))
            .filter(|&column| column != missing)
            .flat_map(|column| (0..4).map(move |y| IVec3::new(column.x, y, column.z)))
            .collect();
        assert_eq!(feed.dirty, expected);

        let mut feed = LightVolumeFeed::default();
        feed.arrive(ColumnPos::new(8, 0), Vec::new(), &scope, |_| true);
        assert!(feed.dirty.iter().all(|section| section.x <= 8));
        assert_eq!(feed.dirty.len(), 2 * 3 * 4);
    }

    #[test]
    fn a_light_update_changes_nothing() {
        let scope = scope(4);
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        let at = ColumnPos::new(0, 0);
        store.insert(at, Column::unlit(0, vec![None, None, None, None]));
        let mut feed = LightVolumeFeed::default();
        feed.arrive(at, vec![2], &scope, |column| store.holds(column));
        let before = (
            feed.emitters.clone(),
            feed.to_build.clone(),
            feed.dirty.clone(),
        );
        let change = ColumnChange::Relit(at, vec![0, 1]);
        feed.observe(&change, &store, &scope, |_: &Column| -> Vec<i32> {
            panic!("a light update reads no column")
        });
        assert_eq!((feed.emitters, feed.to_build, feed.dirty), before);
    }

    #[test]
    fn an_empty_section_of_a_loaded_column_is_air_and_a_missing_column_is_unloaded() {
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        let stone = Section {
            blocks: Box::new([1; SECTION_VOLUME]),
            biomes: Box::new([0; BIOME_CELLS]),
            states: vec![1],
        };
        let at = ColumnPos::new(2, -1);
        store.insert(at, Column::unlit(0, vec![None, Some(stone), None, None]));
        let world = store.around(at);
        for source in [&cells(&store) as &dyn Fn(SectionPos) -> _, &cells(&world)] {
            let air = source(SectionPos::new(2, 0, -1)).expect("a loaded column");
            assert!(air.iter().all(|&block| block == AIR));
            let blocks = source(SectionPos::new(2, 1, -1)).expect("a loaded column");
            assert!(blocks.iter().all(|&block| block == 1));
            assert!(source(SectionPos::new(3, 0, -1)).is_none());
        }
    }

    #[test]
    fn bricks_are_wanted_only_near_emitters_and_one_column_past_the_radius() {
        let scope = scope(2);
        let emitters: HashMap<ColumnPos, Vec<i32>> =
            HashMap::from([(ColumnPos::new(2, 0), vec![3])]);
        let rows = |x, z, loaded| {
            wanted_rows(
                &scope,
                ColumnPos::new(x, z),
                |_| loaded,
                |column| emitters.get(&column).map(Vec::as_slice),
            )
        };
        assert_eq!(rows(2, 0, true), [1, 2, 3]);
        assert_eq!(rows(0, 2, true), [1, 2, 3]);
        assert_eq!(rows(3, 0, true), [1, 2, 3], "one column past the radius");
        assert!(rows(4, 0, true).is_empty(), "two columns past the radius");
        assert!(
            rows(-1, 0, true).is_empty(),
            "three columns from the emitter"
        );
        assert!(rows(2, 0, false).is_empty(), "an unloaded column");

        let emitters: HashMap<ColumnPos, Vec<i32>> =
            HashMap::from([(ColumnPos::new(0, 0), vec![0])]);
        let below = wanted_rows(
            &scope,
            ColumnPos::new(0, 0),
            |_| true,
            |column| emitters.get(&column).map(Vec::as_slice),
        );
        assert_eq!(below, [-1, 0, 1, 2], "the section below the world is built");
    }

    #[test]
    fn a_section_is_dirtied_only_once_its_neighbours_bricks_are_built() {
        let mut feed = LightVolumeFeed::default();
        let section = IVec3::new(4, 1, -2);
        feed.dirty.insert(section);
        feed.to_build.insert(section + IVec3::new(1, 1, -1));
        assert!(feed.take_ready().is_empty());
        assert!(feed.dirty.contains(&section));
        feed.to_build.clear();
        feed.to_build.insert(section + IVec3::new(2, 0, 0));
        assert_eq!(feed.take_ready(), [section]);
        assert!(feed.dirty.is_empty());

        feed.to_build = HashSet::from([IVec3::ZERO]);
        feed.dirty = (0..60).map(|x| IVec3::new(x, 0, 0)).collect();
        let mut ready = feed.take_ready();
        ready.sort_by_key(|section| section.x);
        let expected: Vec<IVec3> = (2..60).map(|x| IVec3::new(x, 0, 0)).collect();
        assert_eq!(ready, expected);
        assert_eq!(feed.dirty, HashSet::from([IVec3::ZERO, IVec3::X]));
    }
}
