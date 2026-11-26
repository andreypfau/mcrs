use crate::fixture::{self, BLOCKS, SPECIAL, TABLES, TYPES};
use glam::DVec3;
use mcrs_minecraft_pathfind::{
    GroundEvaluator, MalusTable, MobFlags, MobInputs, Node, PathTypeTable, SearchRegion,
    SearchScratch,
};
use mcrs_minecraft_physics::{
    Aabb, CollisionSection, CollisionWorld, PathType, SectionLookup, SectionMasks, Tables,
    TestFixture, TestWorld, WorldBorder,
};
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};

struct TallWorld<'a>(&'a TestWorld);

struct TallSection<'a> {
    world: &'a TestWorld,
    origin: BlockPos,
}

impl CollisionWorld for TallWorld<'_> {
    type Section<'a>
        = TallSection<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        SectionLookup::Loaded(TallSection {
            world: self.0,
            origin: at.min_block(),
        })
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.0.border()
    }
}

impl CollisionSection for TallSection<'_> {
    fn masks(&self) -> Option<&SectionMasks> {
        None
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.world.get(
            self.origin
                .offset(i32::from(at.x()), i32::from(at.y()), i32::from(at.z())),
        )
    }
}

fn mob(position: DVec3) -> MobInputs {
    MobInputs::new(
        TestFixture::query_entity(position),
        Aabb::of_body(position, 0.6, 1.95),
        0.6,
        1.95,
    )
}

struct Evaluation {
    start: Node,
    neighbours: Vec<Node>,
}

#[expect(
    clippy::expect_used,
    reason = "test fixtures require an evaluator and its returned nodes"
)]
fn evaluate<W: CollisionWorld>(
    world: &W,
    tables: &Tables,
    types: &PathTypeTable,
    mob: &MobInputs,
    region: &SearchRegion,
) -> Evaluation {
    let mut scratch = SearchScratch::default();
    let mut out = [None; 8];
    let start = {
        let mut evaluator = GroundEvaluator::new(world, tables, types, mob, region, &mut scratch)
            .expect("evaluator");
        let start = evaluator.start();
        evaluator.neighbours(start, &mut out);
        start
    };
    Evaluation {
        start: scratch.node(start).expect("start").clone(),
        neighbours: out
            .into_iter()
            .flatten()
            .map(|id| scratch.node(id).expect("node").clone())
            .collect(),
    }
}

#[test]
fn starts_choose_ground_air_corners_and_the_last_floatable_cell() {
    enum Start {
        Ground,
        Air,
        AboveBuildHeight,
        Corner,
        Bottom,
        Empty,
        Float,
        Overlap,
    }
    struct Case {
        mode: Start,
        expected: BlockPos,
        kind: PathType,
    }
    let cases = [
        Case {
            mode: Start::Ground,
            expected: BlockPos::new(0, 1, 0),
            kind: PathType::Walkable,
        },
        Case {
            mode: Start::Air,
            expected: BlockPos::new(0, 1, 0),
            kind: PathType::Walkable,
        },
        Case {
            mode: Start::AboveBuildHeight,
            expected: BlockPos::new(0, 1, 0),
            kind: PathType::Walkable,
        },
        Case {
            mode: Start::Corner,
            expected: BlockPos::new(-1, 1, -1),
            kind: PathType::Walkable,
        },
        Case {
            mode: Start::Bottom,
            expected: BlockPos::new(0, -2, 0),
            kind: PathType::Open,
        },
        Case {
            mode: Start::Empty,
            expected: BlockPos::new(0, 1, 0),
            kind: PathType::Open,
        },
        Case {
            mode: Start::Float,
            expected: BlockPos::new(0, 3, 0),
            kind: PathType::Water,
        },
        Case {
            mode: Start::Overlap,
            expected: BlockPos::new(0, 0, 0),
            kind: PathType::Blocked,
        },
    ];
    for case in cases {
        let mut world = fixture::floor(0, BLOCKS.stone);
        let mut inputs = mob(DVec3::new(0.5, 1.3, 0.5));
        inputs.on_ground = true;
        match case.mode {
            Start::Ground => {}
            Start::Air => {
                inputs = mob(DVec3::new(0.5, 4.2, 0.5));
            }
            Start::AboveBuildHeight => {
                inputs = mob(DVec3::new(0.5, 10_000_000.0, 0.5));
            }
            Start::Corner => {
                world = TestWorld::new();
                world.set(BlockPos::new(-1, 0, -1), BLOCKS.stone);
                world.set(BlockPos::new(-1, 0, 0), BLOCKS.stone);
                inputs = mob(DVec3::new(0.1, 1.0, 0.1));
                inputs.on_ground = true;
            }
            Start::Bottom => {
                inputs = mob(DVec3::new(0.5, -1.2, 0.5));
            }
            Start::Empty => {
                world = TestWorld::new();
                inputs = mob(DVec3::new(0.5, 4.2, 0.5));
            }
            Start::Float | Start::Overlap => {
                inputs.path.flags = inputs.path.flags.union(MobFlags::FLOAT);
                inputs.in_floatable_fluid = true;
                if matches!(case.mode, Start::Float) {
                    world.fill(BlockPos::new(0, 1, 0), BlockPos::new(0, 3, 0), BLOCKS.water);
                }
            }
        }
        let region = SearchRegion::new(inputs.path.block_position, 16.0, 0..=16);
        let result = evaluate(&world, &TABLES, &TYPES, &inputs, &region);
        assert_eq!(result.start.position, case.expected);
        assert_eq!(result.start.kind, case.kind);
        assert_eq!(result.start.malus, inputs.path.malus.get(case.kind));
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of ground neighbour cases")]
fn neighbours_preserve_order_and_obey_steps_fences_falls_and_corners() {
    enum Terrain {
        Flat,
        Step(i32),
        Sticky,
        Fence,
        Ledge(i32),
        Corner,
        Water,
        Slab,
    }
    struct Case {
        terrain: Terrain,
        step: f32,
        flags: MobFlags,
        north: Option<i32>,
    }
    let cases = [
        Case {
            terrain: Terrain::Flat,
            step: 0.6,
            flags: MobFlags::default(),
            north: Some(1),
        },
        Case {
            terrain: Terrain::Step(1),
            step: 0.6,
            flags: MobFlags::default(),
            north: Some(2),
        },
        Case {
            terrain: Terrain::Step(2),
            step: 0.6,
            flags: MobFlags::default(),
            north: None,
        },
        Case {
            terrain: Terrain::Step(2),
            step: 2.0,
            flags: MobFlags::default(),
            north: Some(3),
        },
        Case {
            terrain: Terrain::Step(2048),
            step: 2048.0,
            flags: MobFlags::default(),
            north: Some(2049),
        },
        Case {
            terrain: Terrain::Sticky,
            step: 0.6,
            flags: MobFlags::default(),
            north: None,
        },
        Case {
            terrain: Terrain::Fence,
            step: 1.5,
            flags: MobFlags::default(),
            north: None,
        },
        Case {
            terrain: Terrain::Fence,
            step: 1.5,
            flags: MobFlags::default().union(MobFlags::WALK_OVER_FENCES),
            north: Some(2),
        },
        Case {
            terrain: Terrain::Ledge(3),
            step: 0.6,
            flags: MobFlags::default(),
            north: Some(1),
        },
        Case {
            terrain: Terrain::Ledge(4),
            step: 0.6,
            flags: MobFlags::default(),
            north: None,
        },
        Case {
            terrain: Terrain::Corner,
            step: 0.6,
            flags: MobFlags::default(),
            north: None,
        },
        Case {
            terrain: Terrain::Water,
            step: 0.6,
            flags: MobFlags::default(),
            north: Some(1),
        },
        Case {
            terrain: Terrain::Water,
            step: 0.6,
            flags: MobFlags::default().union(MobFlags::FLOAT),
            north: Some(3),
        },
        Case {
            terrain: Terrain::Slab,
            step: 0.6,
            flags: MobFlags::default(),
            north: None,
        },
    ];
    for case in cases {
        let mut world = fixture::floor(0, BLOCKS.stone);
        let mut inputs = mob(DVec3::new(0.5, 1.0, 0.5));
        match case.terrain {
            Terrain::Flat => {}
            Terrain::Step(height) => world.fill(
                BlockPos::new(0, 1, -1),
                BlockPos::new(0, height, -1),
                BLOCKS.stone,
            ),
            Terrain::Sticky => {
                world = TestWorld::new();
                world.set(BlockPos::new(0, 0, 0), SPECIAL.id(PathType::Sticky));
                world.set(BlockPos::new(0, 1, -1), SPECIAL.id(PathType::Blocked));
            }
            Terrain::Fence => world.set(BlockPos::new(0, 1, -1), BLOCKS.fence),
            Terrain::Ledge(height) => {
                world.set(BlockPos::new(0, height, 0), BLOCKS.stone);
                inputs = mob(DVec3::new(0.5, f64::from(height) + 1.0, 0.5));
            }
            Terrain::Corner => world.fill(
                BlockPos::new(0, 1, -1),
                BlockPos::new(0, 2, -1),
                BLOCKS.stone,
            ),
            Terrain::Water => {
                world.fill(
                    BlockPos::new(0, 1, -1),
                    BlockPos::new(0, 3, -1),
                    BLOCKS.water,
                );
                world.set(BlockPos::new(0, 2, 0), BLOCKS.stone);
                inputs = mob(DVec3::new(0.5, 3.0, 0.5));
            }
            Terrain::Slab => {
                world.set(BlockPos::new(0, 0, 0), BLOCKS.bottom_slab);
                world.set(BlockPos::new(0, 1, -1), BLOCKS.stone);
                inputs = mob(DVec3::new(0.5, 0.5, 0.5));
            }
        }
        let (tables, types): (&Tables, &PathTypeTable) = if matches!(case.terrain, Terrain::Sticky)
        {
            (&SPECIAL.tables, &SPECIAL.types)
        } else {
            (&TABLES, &TYPES)
        };
        world.build_masks(&tables.collision);
        inputs.on_ground = true;
        inputs.step_height = case.step;
        inputs.path.flags = case.flags;
        let max_y = match case.terrain {
            Terrain::Step(height) => 319.max(height.saturating_add(2)),
            _ => 319,
        };
        let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=max_y);
        let result = if max_y > 319 {
            std::thread::Builder::new()
                .stack_size(128 * 1024)
                .spawn(move || evaluate(&TallWorld(&world), tables, types, &inputs, &region))
                .expect("evaluation thread")
                .join()
                .expect("evaluation result")
        } else {
            evaluate(&world, tables, types, &inputs, &region)
        };
        let north = result
            .neighbours
            .iter()
            .find(|node| node.position.x == 0 && node.position.z == -1);
        assert_eq!(north.map(|node| node.position.y), case.north);
        if matches!(case.terrain, Terrain::Flat) {
            let positions = result
                .neighbours
                .iter()
                .map(|node| node.position)
                .collect::<Vec<_>>();
            assert_eq!(
                positions,
                [
                    (0, 1, -1),
                    (1, 1, 0),
                    (0, 1, 1),
                    (-1, 1, 0),
                    (1, 1, -1),
                    (1, 1, 1),
                    (-1, 1, 1),
                    (-1, 1, -1)
                ]
                .map(BlockPos::from)
            );
        }
        if matches!(case.terrain, Terrain::Corner) {
            assert!(
                !result
                    .neighbours
                    .iter()
                    .any(|node| node.position == BlockPos::new(1, 1, -1))
            );
        }
    }
}

#[test]
fn step_boxes_use_the_mob_context_and_respect_build_height() {
    struct Case {
        fall_distance: f64,
        solid: bool,
        max_y: i32,
        north: Option<i32>,
    }
    let cases = [
        Case {
            fall_distance: 0.0,
            solid: false,
            max_y: 319,
            north: Some(2),
        },
        Case {
            fall_distance: 3.0,
            solid: false,
            max_y: 319,
            north: None,
        },
        Case {
            fall_distance: 0.0,
            solid: true,
            max_y: 319,
            north: None,
        },
        Case {
            fall_distance: 3.0,
            solid: false,
            max_y: 2,
            north: Some(2),
        },
    ];
    for case in cases {
        let mut world = fixture::floor(0, BLOCKS.stone);
        world.set(BlockPos::new(0, 1, -1), BLOCKS.stone);
        world.set(BlockPos::new(0, 3, 0), BLOCKS.powder_snow);
        world.build_masks(&TABLES.collision);
        let mut inputs = mob(DVec3::new(0.5, 1.0, 0.5));
        inputs.on_ground = true;
        inputs.entity.fall_distance = case.fall_distance;
        inputs.entity.powder_snow_solid = case.solid;
        let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=case.max_y);
        let result = evaluate(&world, &TABLES, &TYPES, &inputs, &region);
        let north = result
            .neighbours
            .iter()
            .find(|node| node.position.x == 0 && node.position.z == -1);
        assert_eq!(north.map(|node| node.position.y), case.north);
    }
}

#[test]
fn partial_collision_sweeps_include_the_world_border() {
    for border in [
        None,
        Some(WorldBorder {
            min_x: -8.0,
            max_x: 8.0,
            min_z: 0.0,
            max_z: 8.0,
        }),
    ] {
        let mut world = fixture::floor(0, SPECIAL.id(PathType::Blocked));
        world.set(BlockPos::new(0, 1, 0), SPECIAL.id(PathType::Fence));
        world.set_border(border);
        let mut inputs = mob(DVec3::new(0.5, 1.0, 0.5));
        inputs.on_ground = true;
        inputs.path.malus = MalusTable::new([(PathType::Fence, 0.0)]);
        let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=319);
        let result = evaluate(&world, &SPECIAL.tables, &SPECIAL.types, &inputs, &region);
        assert_eq!(result.start.kind, PathType::Fence);
        assert_eq!(
            result
                .neighbours
                .iter()
                .any(|node| node.position == BlockPos::new(0, 1, -1)),
            border.is_none()
        );
    }
}

#[test]
fn an_excessive_fall_rewrites_a_previously_accepted_node() {
    let mut world = fixture::floor(0, BLOCKS.stone);
    world.set(BlockPos::new(0, 3, 0), BLOCKS.stone);
    world.set(BlockPos::new(1, 4, 0), BLOCKS.stone);
    world.set(BlockPos::new(1, 4, -1), BLOCKS.stone);
    let mut inputs = mob(DVec3::new(0.5, 4.0, 0.5));
    inputs.on_ground = true;
    let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=319);
    let mut scratch = SearchScratch::default();
    let north = {
        let mut evaluator =
            GroundEvaluator::new(&world, &TABLES, &TYPES, &inputs, &region, &mut scratch)
                .expect("evaluator");
        let start = evaluator.start();
        let mut out = [None; 8];
        evaluator.neighbours(start, &mut out);
        let north = out[0].expect("north");
        let east = out[1].expect("east");
        evaluator.neighbours(east, &mut out);
        let high = out[0].expect("high north");
        evaluator.neighbours(high, &mut out);
        assert!(!out.contains(&Some(north)));
        north
    };
    let node = scratch.node(north).expect("stored node");
    assert_eq!(node.position, BlockPos::new(0, 1, -1));
    assert_eq!(node.kind, PathType::Blocked);
    assert_eq!(node.malus, -1.0);
}

#[test]
fn nodes_with_the_same_horizontal_position_keep_their_full_height() {
    let mut world = fixture::floor(0, BLOCKS.stone);
    world.fill(
        BlockPos::new(0, 1, -1),
        BlockPos::new(0, 256, -1),
        BLOCKS.stone,
    );
    let mut inputs = mob(DVec3::new(0.5, 1.0, 0.5));
    inputs.on_ground = true;
    inputs.step_height = 256.0;
    let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=319);
    let mut scratch = SearchScratch::default();
    let start = {
        let mut evaluator =
            GroundEvaluator::new(&world, &TABLES, &TYPES, &inputs, &region, &mut scratch)
                .expect("evaluator");
        let start = evaluator.start();
        let mut out = [None; 8];
        evaluator.neighbours(start, &mut out);
        evaluator.neighbours(out[0].expect("high north"), &mut out);
        start
    };
    let node = scratch.node(start).expect("start node");
    assert_eq!(node.position, BlockPos::new(0, 1, 0));
    assert_eq!(node.kind, PathType::Walkable);
}

#[test]
fn diagonals_use_closed_fences_door_types_width_and_lower_sides() {
    enum Sides {
        Fences,
        Door,
        Fire,
        Lower,
    }
    struct Case {
        sides: Sides,
        width: f32,
        diagonal: bool,
    }
    let cases = [
        Case {
            sides: Sides::Fences,
            width: 0.4,
            diagonal: true,
        },
        Case {
            sides: Sides::Fences,
            width: 0.6,
            diagonal: false,
        },
        Case {
            sides: Sides::Door,
            width: 0.6,
            diagonal: false,
        },
        Case {
            sides: Sides::Fire,
            width: 0.6,
            diagonal: true,
        },
        Case {
            sides: Sides::Fire,
            width: 1.2,
            diagonal: false,
        },
        Case {
            sides: Sides::Lower,
            width: 0.6,
            diagonal: true,
        },
    ];
    for case in cases {
        let mut world = fixture::floor(0, SPECIAL.id(PathType::Blocked));
        let y = if matches!(case.sides, Sides::Lower) {
            5.0
        } else {
            1.0
        };
        let position = DVec3::new(0.5, y, 0.5);
        let mut inputs = MobInputs::new(
            TestFixture::query_entity(position),
            Aabb::of_body(position, case.width, 1.95),
            case.width,
            1.95,
        );
        inputs.on_ground = true;
        match case.sides {
            Sides::Fences => {
                world.set(BlockPos::new(0, 1, -1), SPECIAL.id(PathType::Fence));
                world.set(BlockPos::new(1, 1, 0), SPECIAL.id(PathType::Fence));
            }
            Sides::Door => {
                inputs.path.flags = inputs.path.flags.union(MobFlags::OPEN_DOORS);
                world.set(
                    BlockPos::new(0, 1, -1),
                    SPECIAL.id(PathType::DoorWoodClosed),
                );
            }
            Sides::Fire => world.set(BlockPos::new(0, 0, -1), SPECIAL.id(PathType::Fire)),
            Sides::Lower => {
                world.set(BlockPos::new(0, 4, 0), SPECIAL.id(PathType::Blocked));
                world.set(BlockPos::new(1, 4, 0), SPECIAL.id(PathType::Blocked));
                world.set(BlockPos::new(1, 1, -1), SPECIAL.id(PathType::Blocked));
            }
        }
        let region = SearchRegion::new(inputs.path.block_position, 16.0, -64..=319);
        let found = evaluate(&world, &SPECIAL.tables, &SPECIAL.types, &inputs, &region)
            .neighbours
            .iter()
            .any(|node| node.position.x == 1 && node.position.z == -1);
        assert_eq!(found, case.diagonal);
    }
}
