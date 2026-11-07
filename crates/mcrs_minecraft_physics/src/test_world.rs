use std::collections::HashMap;
use std::collections::HashSet;

use glam::DVec3;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::Aabb;
use crate::BlockFlags;
use crate::BlockMotion;
use crate::CollisionSection;
use crate::CollisionTable;
use crate::CollisionTableBuilder;
use crate::CollisionWorld;
use crate::ContextShape;
use crate::InsideEffect;
use crate::SectionLookup;
use crate::SectionMasks;
use crate::StateSpec;
use crate::WorldBorder;

/// A block world in a hash map, for tests and benchmarks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestWorld {
    blocks: HashMap<BlockPos, BlockStateId>,
    unloaded: HashSet<(i32, i32)>,
    masks: HashMap<SectionPos, SectionMasks>,
    border: Option<WorldBorder>,
    min_y: i32,
    max_y: i32,
}

impl TestWorld {
    /// An empty world from `y = -64` to `319`, with every chunk loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_y: -64,
            max_y: 319,
            ..Self::default()
        }
    }

    pub fn set(&mut self, at: BlockPos, state: BlockStateId) {
        if state == BlockStateId::AIR {
            self.blocks.remove(&at);
        } else {
            self.blocks.insert(at, state);
        }
        self.masks.clear();
    }

    /// Fills the box of blocks from `min` to `max`, both included.
    pub fn fill(&mut self, min: BlockPos, max: BlockPos, state: BlockStateId) {
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    self.set(BlockPos::new(x, y, z), state);
                }
            }
        }
    }

    #[must_use]
    pub fn get(&self, at: BlockPos) -> BlockStateId {
        self.blocks.get(&at).copied().unwrap_or(BlockStateId::AIR)
    }

    pub fn unload_chunk(&mut self, chunk_x: i32, chunk_z: i32) {
        self.unloaded.insert((chunk_x, chunk_z));
    }

    pub fn set_border(&mut self, border: Option<WorldBorder>) {
        self.border = border;
    }

    /// Gives each section with a block its masks, so the scan takes the masked path.
    pub fn build_masks(&mut self, table: &CollisionTable) {
        let sections: HashSet<SectionPos> =
            self.blocks.keys().map(|&at| SectionPos::from(at)).collect();
        let mut masks = HashMap::new();
        for section in sections {
            let origin = BlockPos::new(section.x << 4, section.y << 4, section.z << 4);
            let built = SectionMasks::from_states(table, 0, |local| {
                self.get(BlockPos::new(
                    origin.x + i32::from(local.x()),
                    origin.y + i32::from(local.y()),
                    origin.z + i32::from(local.z()),
                ))
            });
            masks.insert(section, built);
        }
        self.masks = masks;
    }
}

/// A section of a [`TestWorld`].
#[derive(Clone, Copy, Debug)]
pub struct TestSection<'a> {
    world: &'a TestWorld,
    origin: BlockPos,
    masks: Option<&'a SectionMasks>,
}

impl CollisionSection for TestSection<'_> {
    fn masks(&self) -> Option<&SectionMasks> {
        self.masks
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.world.get(BlockPos::new(
            self.origin.x + i32::from(at.x()),
            self.origin.y + i32::from(at.y()),
            self.origin.z + i32::from(at.z()),
        ))
    }
}

impl CollisionWorld for TestWorld {
    type Section<'a> = TestSection<'a>;

    fn section(&self, at: SectionPos) -> SectionLookup<TestSection<'_>> {
        let origin = BlockPos::new(at.x << 4, at.y << 4, at.z << 4);
        if origin.y + 15 < self.min_y || origin.y > self.max_y {
            return SectionLookup::Outside;
        }
        if self.unloaded.contains(&(at.x, at.z)) {
            return SectionLookup::Unloaded;
        }
        SectionLookup::Loaded(TestSection {
            world: self,
            origin,
            masks: if self.masks.is_empty() {
                None
            } else {
                Some(self.masks.get(&at).unwrap_or(&EMPTY_MASKS))
            },
        })
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.border.as_ref()
    }
}

static EMPTY_MASKS: SectionMasks = SectionMasks {
    solid: 0,
    full: 0,
    large: 0,
    action: 0,
    coarse: 0,
    revision: 0,
    special: 0,
    air_only: true,
};

/// A small block table with one state of each shape class, for tests.
#[derive(Clone, Debug, PartialEq)]
pub struct TestBlocks {
    pub table: CollisionTable,
    pub stone: BlockStateId,
    pub bottom_slab: BlockStateId,
    pub top_slab: BlockStateId,
    pub carpet: BlockStateId,
    pub fence: BlockStateId,
    pub wall: BlockStateId,
    pub stairs: BlockStateId,
    /// Snow layers 1 to 8, at index `layers - 1`.
    pub snow: [BlockStateId; 8],
    pub off_grid: BlockStateId,
    pub ice: BlockStateId,
    pub slime: BlockStateId,
    pub soul_sand: BlockStateId,
    pub honey: BlockStateId,
    pub bed: BlockStateId,
    pub cobweb: BlockStateId,
    pub sweet_berry_bush: BlockStateId,
    pub scaffolding: BlockStateId,
    pub scaffolding_bottom: BlockStateId,
    pub powder_snow: BlockStateId,
    pub water: BlockStateId,
}

const PIXEL: f64 = 1.0 / 16.0;

fn cube(min: [f64; 3], max: [f64; 3]) -> Aabb {
    Aabb::new(DVec3::from_array(min), DVec3::from_array(max))
}

impl TestBlocks {
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one flat list of the test states")]
    pub fn new() -> Self {
        let mut builder = CollisionTableBuilder::new();
        let mut push = |boxes: &[Aabb], flags: BlockFlags, motion: BlockMotion| {
            builder
                .push(StateSpec {
                    boxes,
                    context: None,
                    flags,
                    inside: InsideEffect::None,
                    motion,
                })
                .unwrap_or(BlockStateId::AIR)
        };
        let plain = BlockFlags::empty();
        let default = BlockMotion::DEFAULT;
        let _air = push(&[], plain, default);
        let stone = push(&[cube([0.0; 3], [1.0; 3])], BlockFlags::SUFFOCATES, default);
        let bottom_slab = push(&[cube([0.0; 3], [1.0, 0.5, 1.0])], plain, default);
        let top_slab = push(&[cube([0.0, 0.5, 0.0], [1.0; 3])], plain, default);
        let carpet = push(&[cube([0.0; 3], [1.0, PIXEL, 1.0])], plain, default);
        let fence = push(
            &[cube(
                [6.0 * PIXEL, 0.0, 6.0 * PIXEL],
                [10.0 * PIXEL, 1.5, 10.0 * PIXEL],
            )],
            BlockFlags::FENCE,
            default,
        );
        let wall = push(
            &[cube(
                [4.0 * PIXEL, 0.0, 4.0 * PIXEL],
                [12.0 * PIXEL, 1.5, 12.0 * PIXEL],
            )],
            BlockFlags::WALL,
            default,
        );
        let stairs = push(
            &[
                cube([0.0; 3], [1.0, 0.5, 1.0]),
                cube([0.0, 0.5, 0.5], [1.0; 3]),
            ],
            plain,
            default,
        );
        let snow = core::array::from_fn(|index| {
            let height = (index as f64 + 1.0) * 2.0 * PIXEL;
            push(&[cube([0.0; 3], [1.0, height, 1.0])], plain, default)
        });
        let off_grid = push(
            &[cube([0.3; 3], [0.7, 0.45, 0.7])],
            BlockFlags::OFF_GRID,
            default,
        );
        let ice = push(
            &[cube([0.0; 3], [1.0; 3])],
            plain,
            BlockMotion {
                friction: 0.98,
                ..default
            },
        );
        let slime = push(
            &[cube([0.0; 3], [1.0; 3])],
            BlockFlags::SLIME | BlockFlags::STEP_ACTION,
            BlockMotion {
                friction: 0.8,
                bounce: 1.0,
                ..default
            },
        );
        let soul_sand = push(
            &[cube([0.0; 3], [1.0, 14.0 * PIXEL, 1.0])],
            plain,
            BlockMotion {
                speed_factor: 0.4,
                ..default
            },
        );
        let honey = push(
            &[cube(
                [PIXEL, 0.0, PIXEL],
                [15.0 * PIXEL, 15.0 * PIXEL, 15.0 * PIXEL],
            )],
            BlockFlags::SUPPRESSES_BOUNCE,
            BlockMotion {
                speed_factor: 0.4,
                jump_factor: 0.5,
                ..default
            },
        );
        let bed = push(
            &[cube([0.0; 3], [1.0, 9.0 * PIXEL, 1.0])],
            plain,
            BlockMotion {
                bounce: 0.75,
                ..default
            },
        );
        let mut push_special =
            |context: Option<ContextShape>, flags: BlockFlags, inside: InsideEffect| {
                builder
                    .push(StateSpec {
                        boxes: &[],
                        context,
                        flags,
                        inside,
                        motion: default,
                    })
                    .unwrap_or(BlockStateId::AIR)
            };
        let cobweb = push_special(
            None,
            BlockFlags::INSIDE_ACTION | BlockFlags::FALL_DAMAGE_RESETTING,
            InsideEffect::Cobweb,
        );
        let sweet_berry_bush = push_special(
            None,
            BlockFlags::INSIDE_ACTION | BlockFlags::FALL_DAMAGE_RESETTING,
            InsideEffect::SweetBerryBush,
        );
        let scaffolding = push_special(
            Some(ContextShape::Scaffolding { bottom: false }),
            BlockFlags::CLIMBABLE,
            InsideEffect::None,
        );
        let scaffolding_bottom = push_special(
            Some(ContextShape::Scaffolding { bottom: true }),
            BlockFlags::CLIMBABLE,
            InsideEffect::None,
        );
        let powder_snow = push_special(
            Some(ContextShape::PowderSnow),
            BlockFlags::INSIDE_ACTION,
            InsideEffect::PowderSnow,
        );
        let water = push_special(
            None,
            BlockFlags::FLUID | BlockFlags::OWN_SPEED_FACTOR | BlockFlags::COARSE_NEAR,
            InsideEffect::None,
        );
        Self {
            table: builder.build(),
            stone,
            bottom_slab,
            top_slab,
            carpet,
            fence,
            wall,
            stairs,
            snow,
            off_grid,
            ice,
            slime,
            soul_sand,
            honey,
            bed,
            cobweb,
            sweet_berry_bush,
            scaffolding,
            scaffolding_bottom,
            powder_snow,
            water,
        }
    }

    /// Every state with a collision shape for no context.
    #[must_use]
    pub fn solid_states(&self) -> Vec<BlockStateId> {
        let mut states = vec![
            self.stone,
            self.bottom_slab,
            self.top_slab,
            self.carpet,
            self.fence,
            self.wall,
            self.stairs,
            self.off_grid,
            self.ice,
            self.slime,
            self.soul_sand,
            self.honey,
            self.bed,
        ];
        states.extend(self.snow);
        states
    }
}

impl Default for TestBlocks {
    fn default() -> Self {
        Self::new()
    }
}
