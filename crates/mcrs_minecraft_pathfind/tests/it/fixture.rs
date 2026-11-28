use glam::DVec3;
use mcrs_minecraft_pathfind::{PathTypeTable, StateFlags};
use mcrs_minecraft_physics::{
    Aabb, BlockFlags, BlockMotion, CollisionTable, CollisionTableBuilder, InsideBoxes,
    InsideEffect, PathType, StateSpec, Tables, TestBlocks, TestWorld,
};
use mcrs_voxel::{BlockPos, BlockStateId};
use std::collections::HashMap;
use std::sync::LazyLock;

pub static BLOCKS: LazyLock<TestBlocks> = LazyLock::new(TestBlocks::new);
pub static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables::new(BLOCKS.table.clone()));
pub static TYPES: LazyLock<PathTypeTable> =
    LazyLock::new(|| table(&TABLES.collision, Some(BLOCKS.water)));
pub static SPECIAL: LazyLock<Special> = LazyLock::new(Special::new);

#[expect(clippy::expect_used, reason = "test flags cover every collision state")]
pub fn table(collision: &CollisionTable, water: Option<BlockStateId>) -> PathTypeTable {
    let flags = (0..collision.len())
        .map(|index| {
            if index == 0 {
                StateFlags::AIR.union(StateFlags::LAND_PATHFINDABLE)
            } else if water.is_some_and(|water| water.index() == index) {
                StateFlags::FLOATABLE_FLUID.union(StateFlags::LAND_PATHFINDABLE)
            } else {
                StateFlags::default()
            }
        })
        .collect::<Vec<_>>();
    PathTypeTable::new(collision, &flags).expect("flags cover every collision state")
}

pub fn floor(y: i32, state: BlockStateId) -> TestWorld {
    let mut world = TestWorld::new();
    world.fill(BlockPos::new(-3, y, -3), BlockPos::new(3, y, 3), state);
    world
}

pub struct Special {
    pub types: PathTypeTable,
    states: HashMap<PathType, BlockStateId>,
}

#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test fixtures require the states they define"
)]
impl Special {
    fn new() -> Self {
        use PathType::*;
        let mut builder = CollisionTableBuilder::new();
        let states = [
            Open,
            Blocked,
            Fire,
            DropDown,
            Water,
            DoorWoodClosed,
            DoorOpen,
            Rail,
            Damaging,
            Sticky,
            PowderSnow,
            Fence,
            DoorIronClosed,
            Lava,
            DamageCautious,
            Leaves,
        ]
        .into_iter()
        .map(|path| {
            let boxes = match path {
                Blocked | Fire | Sticky => vec![Aabb::new(DVec3::ZERO, DVec3::ONE)],
                DoorWoodClosed | DoorIronClosed | DropDown => {
                    vec![Aabb::new(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.1875))]
                }
                Fence => vec![Aabb::new(
                    DVec3::splat(0.375),
                    DVec3::new(0.625, 1.5, 0.625),
                )],
                _ => Vec::new(),
            };
            let id = builder
                .push(StateSpec {
                    boxes: &boxes,
                    context: None,
                    flags: BlockFlags::empty(),
                    inside: InsideEffect::None,
                    inside_shape: InsideBoxes::FullCube,
                    motion: BlockMotion::DEFAULT,
                    path,
                })
                .expect("test state");
            (path, id)
        })
        .collect::<HashMap<_, _>>();
        let tables = Tables::new(builder.build());
        let types = table(&tables.collision, states.get(&Water).copied());
        Self { types, states }
    }
    pub fn id(&self, kind: PathType) -> BlockStateId {
        self.states[&kind]
    }
}
