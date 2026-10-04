use std::ops::RangeInclusive;
use std::sync::Arc;

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
use mcrs_minecraft_render::CameraOrigin;
use mcrs_minecraft_render_light_color::{
    LightVolumeRenderPlugin, VolumeCommand, VolumeQueue, VolumeSettings,
};

use crate::Unsettled;
use crate::columns::{
    AIR, BlockSource, ClientTerrainSet, Column, ColumnChange, ColumnStore, Extent, SECTION_VOLUME,
};

static AIR_SECTION: [u16; SECTION_VOLUME] = [AIR; SECTION_VOLUME];

pub struct LightVolumePlugin {
    /// Columns around the camera whose block light is coloured; 0 is off.
    pub radius: u8,
    /// Sections the GPU colours in one frame.
    pub sections_per_frame: u32,
    /// Section bricks that may start building in one frame.
    pub bricks_per_frame: usize,
}

impl Default for LightVolumePlugin {
    #[cfg(not(target_family = "wasm"))]
    fn default() -> Self {
        Self {
            radius: 10,
            sections_per_frame: 8,
            bricks_per_frame: 512,
        }
    }

    #[cfg(target_family = "wasm")]
    fn default() -> Self {
        Self {
            radius: 6,
            sections_per_frame: 2,
            bricks_per_frame: 16,
        }
    }
}

impl Plugin for LightVolumePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(mcrs_minecraft_light_color::plugin::LightColorPlugin)
            .add_plugins(LightVolumeRenderPlugin {
                settings: VolumeSettings {
                    radius: self.radius,
                    view_distance: crate::config::view_distance(),
                    sections_per_frame: self.sections_per_frame,
                },
            })
            .insert_resource(BricksPerFrame(self.bricks_per_frame))
            .init_resource::<LightVolumeFeed>()
            .add_message::<Unsettled>()
            .add_systems(
                Update,
                (
                    feed_light_volume.run_if(resource_exists::<ColumnStore>),
                    report_unsettled,
                )
                    .chain()
                    .in_set(ClientTerrainSet::Build),
            );
    }
}

#[derive(Resource)]
struct BricksPerFrame(usize);

/// What the feeder has handed the renderer and what it still owes it, kept only while the
/// light volume is open. The column store stays the truth.
#[derive(Resource, Default)]
pub struct LightVolumeFeed {
    active: bool,
    generation: u64,
    extent: Option<Extent>,
    camera: ColumnPos,
    emitters: HashMap<ColumnPos, Vec<i32>>,
    built: HashMap<IVec3, Vec<Emitter>>,
    to_build: HashSet<IVec3>,
    building: HashMap<IVec3, Task<PackedBrick>>,
    dirty: HashSet<IVec3>,
    evicting: Vec<IVec3>,
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
            && self.world_rows().contains(&section.y)
    }

    fn world_rows(&self) -> RangeInclusive<i32> {
        self.extent.min_section_y..=self.max_y()
    }

    fn around(&self, reach: i32) -> impl Iterator<Item = ColumnPos> + use<> {
        let camera = column_of(self.camera);
        (-reach..=reach).flat_map(move |dz| {
            (-reach..=reach).map(move |dx| ColumnPos::new(camera.x + dx, camera.z + dz))
        })
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
    /// Nothing to build, nothing building and nothing dirty; a feeder that is not running holds
    /// nothing and so is idle.
    pub fn idle(&self) -> bool {
        self.to_build.is_empty() && self.building.is_empty() && self.dirty.is_empty()
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn start(
        &mut self,
        generation: u64,
        scope: &Scope,
        store: &ColumnStore,
        rows: impl Fn(&Column) -> Vec<i32>,
        queue: &VolumeQueue,
    ) {
        self.clear();
        self.active = true;
        self.generation = generation;
        self.extent = Some(scope.extent);
        self.camera = column_of(scope.camera);
        queue.push(VolumeCommand::Reset {
            min_section_y: scope.extent.min_section_y,
            sections: scope.extent.sections as u32,
        });
        let mut resident: Vec<ColumnPos> = store.resident().map(|(at, _)| at).collect();
        resident.sort_unstable_by_key(|&at| scope.columns_from_camera(at));
        for at in resident {
            if let Some(column) = store.get(at) {
                self.arrive(at, rows(column), scope, |c| store.holds(c));
            }
        }
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

    /// Evicts the column's bricks and whatever colour it may hold. Its neighbours keep the colour
    /// its light gave them.
    fn depart(&mut self, at: ColumnPos, scope: &Scope) {
        self.emitters.remove(&at);
        self.to_build.retain(|&section| column_of(section) != at);
        self.building.retain(|&section, _| column_of(section) != at);
        self.dirty.retain(|&section| column_of(section) != at);
        for y in scope.brick_rows() {
            let section = IVec3::new(at.x, y, at.z);
            if self.built.remove(&section).is_some() || scope.colours(section) {
                self.evicting.push(section);
            }
        }
    }

    /// Bricks more than one column past the radius are evicted, those coming within it are built,
    /// and sections coming inside the radius are dirtied. The renderer drops the colour of
    /// sections leaving the radius itself.
    fn follow(&mut self, scope: &Scope, loaded: impl Fn(ColumnPos) -> bool) {
        let camera = column_of(scope.camera);
        if camera == self.camera {
            return;
        }
        let before = Scope {
            camera: IVec3::new(self.camera.x, 0, self.camera.z),
            ..*scope
        };
        self.camera = camera;
        let reach = scope.radius + 1;
        let beyond = |section: IVec3| scope.columns_from_camera(column_of(section)) > reach;
        let evicting = &mut self.evicting;
        self.built.retain(|&section, _| {
            let keep = !beyond(section);
            if !keep {
                evicting.push(section);
            }
            keep
        });
        self.to_build.retain(|&section| !beyond(section));
        self.building.retain(|&section, _| !beyond(section));
        self.dirty.retain(|&section| scope.colours(section));
        for column in scope.around(reach) {
            if before.columns_from_camera(column) <= reach {
                continue;
            }
            let rows = wanted_rows(scope, column, &loaded, |around| {
                self.emitters.get(&around).map(Vec::as_slice)
            });
            for y in rows {
                let section = IVec3::new(column.x, y, column.z);
                if !self.built.contains_key(&section) {
                    self.to_build.insert(section);
                }
            }
        }
        for column in scope.around(scope.radius) {
            if before.columns_from_camera(column) > scope.radius && loaded(column) {
                self.dirty.extend(
                    scope
                        .world_rows()
                        .map(|y| IVec3::new(column.x, y, column.z)),
                );
            }
        }
    }

    /// A block's light type may have changed with the colours, so every wanted brick is built
    /// again and every section in the radius coloured again.
    fn recolour(&mut self, scope: &Scope, loaded: impl Fn(ColumnPos) -> bool) {
        self.to_build.extend(self.built.keys().copied());
        self.to_build
            .extend(self.building.drain().map(|(section, _)| section));
        for column in scope.around(scope.radius).filter(|&column| loaded(column)) {
            self.dirty.extend(
                scope
                    .world_rows()
                    .map(|y| IVec3::new(column.x, y, column.z)),
            );
        }
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
            ColumnChange::Departed(at, _) => self.depart(*at, scope),
            ColumnChange::Relit(..) => {}
        }
    }

    fn land(&mut self, queue: &VolumeQueue) {
        for section in self.evicting.drain(..) {
            queue.push(VolumeCommand::Evict { section });
        }
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
    queue: Res<VolumeQueue>,
    origin: Res<CameraOrigin>,
    colours: Option<Res<LightColors>>,
    blocks: Option<Res<Blocks>>,
    bricks: Res<BricksPerFrame>,
) {
    let radius = queue.radius();
    let (Some(colours), Some(blocks), Some(extent), 1..) =
        (colours, blocks, store.extent(), radius)
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
    if !feed.active || feed.generation != generation || feed.extent != Some(extent) {
        changes.clear();
        feed.start(generation, &scope, &store, rows, &queue);
    } else {
        for change in changes.read() {
            feed.observe(change, &store, &scope, rows);
        }
        feed.follow(&scope, |c| store.holds(c));
        if colours.is_changed() {
            feed.recolour(&scope, |c| store.holds(c));
        }
    }
    feed.land(&queue);
    feed.admit(&scope, &store, &registry, &colours, bricks.0);
    feed.push_dirty(&colours, &queue);
}

fn report_unsettled(
    feed: Res<LightVolumeFeed>,
    queue: Res<VolumeQueue>,
    mut unsettled: MessageWriter<Unsettled>,
) {
    if !(feed.idle() && queue.idle()) {
        unsettled.write(Unsettled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::{Dimension, Section};
    use mcrs_minecraft_chunk::PalettedContainer;
    use mcrs_minecraft_light_color::colors::LightType;
    use std::time::Duration;

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
            biomes: PalettedContainer::Homogeneous(0),
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Sent {
        Reset(i32, u32),
        Brick(IVec3),
        Evict(IVec3),
        Dirty(IVec3),
    }

    fn sent(command: &VolumeCommand) -> Sent {
        match command {
            VolumeCommand::Reset {
                min_section_y,
                sections,
            } => Sent::Reset(*min_section_y, *sections),
            VolumeCommand::Brick { section, .. } => Sent::Brick(*section),
            VolumeCommand::Evict { section } => Sent::Evict(*section),
            VolumeCommand::Dirty { section, .. } => Sent::Dirty(*section),
        }
    }

    fn publish_changes(mut store: ResMut<ColumnStore>, mut changes: MessageWriter<ColumnChange>) {
        let mut drained = Vec::new();
        store.bypass_change_detection().drain_changes(&mut drained);
        changes.write_batch(drained);
    }

    fn glowstone() -> u16 {
        crate::blocks::corpus()
            .default_state("minecraft:glowstone")
            .0
    }

    fn light_colours(rgb: [u8; 3]) -> LightColors {
        let lamp = glowstone() as usize;
        let mut types = vec![LightType::DEFAULT; lamp + 1];
        types[lamp] = LightType(1);
        LightColors::new(vec![rgb], types)
    }

    /// A column of `extent` with one glowstone block in each of the `lit` rows.
    fn column_in(extent: Extent, lit: &[i32]) -> Column {
        let lamp = glowstone();
        let sections = extent.min_section_y..extent.min_section_y + extent.sections as i32;
        let sections = sections
            .map(|y| {
                lit.contains(&y).then(|| {
                    let mut blocks = Box::new([AIR; SECTION_VOLUME]);
                    blocks[0] = lamp;
                    Section {
                        blocks,
                        biomes: PalettedContainer::Homogeneous(0),
                        states: vec![AIR, lamp],
                    }
                })
            })
            .collect();
        Column::unlit(extent.min_section_y, sections)
    }

    fn column(lit: &[i32]) -> Column {
        column_in(EXTENT, lit)
    }

    fn app(radius: u8) -> App {
        let mut store = ColumnStore::default();
        store.enter(EXTENT);
        let queue = VolumeQueue::default();
        queue.restart(radius);
        let mut app = App::new();
        app.add_message::<ColumnChange>()
            .insert_resource(store)
            .insert_resource(queue)
            .insert_resource(crate::blocks::corpus_blocks().clone())
            .insert_resource(light_colours([255, 64, 0]))
            .insert_resource(BricksPerFrame(
                LightVolumePlugin::default().bricks_per_frame,
            ))
            .init_resource::<CameraOrigin>()
            .init_resource::<LightVolumeFeed>()
            .add_systems(Update, (publish_changes, feed_light_volume).chain());
        app
    }

    fn store(app: &mut App) -> Mut<'_, ColumnStore> {
        app.world_mut().resource_mut::<ColumnStore>()
    }

    /// Every command the feeder sends until it has nothing left to build or dirty.
    fn settle_commands(app: &mut App) -> Vec<VolumeCommand> {
        let queue = app.world().resource::<VolumeQueue>().clone();
        let mut commands = Vec::new();
        for _ in 0..5000 {
            app.update();
            queue.take(&mut commands);
            if app.world().resource::<LightVolumeFeed>().idle() {
                return commands;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("the feeder did not settle");
    }

    fn settle(app: &mut App) -> Vec<Sent> {
        settle_commands(app).iter().map(sent).collect()
    }

    fn picked(sent: &[Sent], pick: impl Fn(Sent) -> Option<IVec3>) -> HashSet<IVec3> {
        sent.iter().filter_map(|&command| pick(command)).collect()
    }

    fn bricks(sent: &[Sent]) -> HashSet<IVec3> {
        picked(sent, |command| match command {
            Sent::Brick(section) => Some(section),
            _ => None,
        })
    }

    fn evicted(sent: &[Sent]) -> HashSet<IVec3> {
        picked(sent, |command| match command {
            Sent::Evict(section) => Some(section),
            _ => None,
        })
    }

    fn dirtied(sent: &[Sent]) -> HashSet<IVec3> {
        picked(sent, |command| match command {
            Sent::Dirty(section) => Some(section),
            _ => None,
        })
    }

    fn rows_of(columns: &[ColumnPos], rows: RangeInclusive<i32>) -> HashSet<IVec3> {
        columns
            .iter()
            .flat_map(|column| rows.clone().map(|y| IVec3::new(column.x, y, column.z)))
            .collect()
    }

    fn row(xs: RangeInclusive<i32>) -> Vec<ColumnPos> {
        xs.map(|x| ColumnPos::new(x, 0)).collect()
    }

    #[test]
    fn the_feeder_follows_columns_the_camera_and_the_colours() {
        a_departed_column_evicts_every_brick_it_built();
        moving_the_camera_evicts_what_leaves_and_builds_what_enters_the_radius();
        switching_off_and_on_resets_and_rescans_without_replaying_old_changes();
        a_new_extent_resets_the_volume();
        reloaded_colours_rebuild_and_recolour_the_radius();
    }

    fn a_departed_column_evicts_every_brick_it_built() {
        let mut app = app(2);
        let (lit, plain) = (ColumnPos::new(1, 0), ColumnPos::new(0, 0));
        store(&mut app).insert(lit, column(&[1]));
        store(&mut app).insert(plain, column(&[]));
        let joined = settle(&mut app);
        assert_eq!(joined[0], Sent::Reset(0, 4));
        let own = rows_of(&[lit], -1..=3);
        let neighbour = rows_of(&[plain], -1..=3);
        assert_eq!(bricks(&joined), &own | &neighbour);

        store(&mut app).remove(lit);
        let left = settle(&mut app);
        let expected: HashSet<Sent> = own.iter().map(|&section| Sent::Evict(section)).collect();
        assert_eq!(left.iter().copied().collect::<HashSet<_>>(), expected);
        assert_eq!(left.len(), expected.len(), "each brick is evicted once");
        let feed = app.world().resource::<LightVolumeFeed>();
        assert!(!feed.emitters.contains_key(&lit));
        assert_eq!(
            feed.built.keys().copied().collect::<HashSet<_>>(),
            neighbour,
            "the neighbour keeps the bricks the departed light reached"
        );
    }

    fn moving_the_camera_evicts_what_leaves_and_builds_what_enters_the_radius() {
        let mut app = app(1);
        for column in row(-1..=5) {
            store(&mut app).insert(column, column_lit());
        }
        let joined = settle(&mut app);
        assert_eq!(bricks(&joined), rows_of(&row(-1..=2), -1..=3));
        assert_eq!(dirtied(&joined), rows_of(&row(-1..=1), 0..=3));

        app.world_mut().resource_mut::<CameraOrigin>().section = IVec3::new(3, 1, 0);
        let moved = settle(&mut app);
        assert!(
            !moved
                .iter()
                .any(|command| matches!(command, Sent::Reset(..)))
        );
        assert_eq!(evicted(&moved), rows_of(&row(-1..=0), -1..=3));
        assert_eq!(bricks(&moved), rows_of(&row(3..=5), -1..=3));
        assert_eq!(dirtied(&moved), rows_of(&row(2..=4), 0..=3));
        let evicts = moved
            .iter()
            .filter(|command| matches!(command, Sent::Evict(_)))
            .count();
        assert_eq!(evicts, 10, "each brick is evicted once");
    }

    fn column_lit() -> Column {
        column(&[1])
    }

    fn switching_off_and_on_resets_and_rescans_without_replaying_old_changes() {
        let mut app = app(1);
        let (kept, gone, new) = (
            ColumnPos::new(0, 0),
            ColumnPos::new(1, 0),
            ColumnPos::new(-1, 0),
        );
        store(&mut app).insert(kept, column_lit());
        store(&mut app).insert(gone, column_lit());
        settle(&mut app);

        let queue = app.world().resource::<VolumeQueue>().clone();
        queue.close();
        app.update();
        let feed = app.world().resource::<LightVolumeFeed>();
        assert!(!feed.active && feed.built.is_empty() && feed.emitters.is_empty());

        store(&mut app).insert(kept, column_lit());
        store(&mut app).remove(gone);
        app.update();
        let mut commands = Vec::new();
        queue.take(&mut commands);
        assert!(
            commands.is_empty(),
            "nothing is sent while the volume is closed"
        );

        store(&mut app).insert(new, column_lit());
        queue.restart(1);
        let back = settle(&mut app);
        assert_eq!(back[0], Sent::Reset(0, 4));
        assert_eq!(
            back.iter()
                .filter(|command| matches!(command, Sent::Reset(..)))
                .count(),
            1
        );
        assert!(evicted(&back).is_empty(), "the departure is not replayed");
        let built: Vec<Sent> = back
            .iter()
            .copied()
            .filter(|command| matches!(command, Sent::Brick(_)))
            .collect();
        assert_eq!(bricks(&back), rows_of(&[kept, new], -1..=3));
        assert_eq!(built.len(), bricks(&back).len(), "each brick is built once");
    }

    fn a_new_extent_resets_the_volume() {
        let mut app = app(1);
        let (lit, plain) = (ColumnPos::new(0, 0), ColumnPos::new(1, 0));
        store(&mut app).insert(lit, column_lit());
        store(&mut app).insert(plain, column(&[]));
        settle(&mut app);

        store(&mut app).enter(Dimension {
            name: "minecraft:the_nether".into(),
            extent: EXTENT,
        });
        store(&mut app).insert(lit, column(&[]));
        let same = settle(&mut app);
        assert!(
            !same
                .iter()
                .any(|command| matches!(command, Sent::Reset(..)))
        );
        assert_eq!(evicted(&same), rows_of(&[lit, plain], -1..=3));
        assert!(bricks(&same).is_empty());
        assert_eq!(dirtied(&same), rows_of(&[lit], 0..=3));

        let tall = Extent {
            min_section_y: -2,
            sections: 6,
        };
        store(&mut app).enter(Dimension {
            name: "minecraft:overworld".into(),
            extent: tall,
        });
        store(&mut app).insert(lit, column_in(tall, &[1]));
        let taller = settle(&mut app);
        assert_eq!(taller[0], Sent::Reset(-2, 6));
        assert!(
            evicted(&taller).is_empty(),
            "the reset already forgot every section"
        );
        assert_eq!(bricks(&taller), rows_of(&[lit], -1..=3));
        assert_eq!(dirtied(&taller), rows_of(&[lit], -2..=3));
    }

    fn reloaded_colours_rebuild_and_recolour_the_radius() {
        let mut app = app(1);
        store(&mut app).insert(ColumnPos::new(0, 0), column_lit());
        store(&mut app).insert(ColumnPos::new(1, 0), column(&[]));
        store(&mut app).insert(ColumnPos::new(3, 0), column_lit());
        let joined = settle(&mut app);

        let blue = [0, 128, 255];
        app.world_mut().insert_resource(light_colours(blue));
        let commands = settle_commands(&mut app);
        let reloaded: Vec<Sent> = commands.iter().map(sent).collect();
        assert!(
            !reloaded
                .iter()
                .any(|command| matches!(command, Sent::Reset(..) | Sent::Evict(_)))
        );
        assert_eq!(bricks(&reloaded), bricks(&joined));
        assert_eq!(dirtied(&reloaded), dirtied(&joined));
        assert_eq!(
            dirtied(&reloaded),
            rows_of(&[ColumnPos::new(0, 0), ColumnPos::new(1, 0)], 0..=3)
        );
        for command in &commands {
            if let VolumeCommand::Dirty { lanes, .. } = command {
                for lane in lanes.iter().filter(|lane| lane.light_type == LightType(1)) {
                    assert_eq!(lane.colour, Some(blue));
                }
            }
        }
        assert!(
            commands.iter().any(
                |command| matches!(command, VolumeCommand::Dirty { lanes, .. } if !lanes.is_empty())
            ),
            "some section is coloured by the lamp"
        );
    }
}
