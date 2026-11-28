use crate::fixture::{self, BLOCKS, SPECIAL, TYPES};
use mcrs_minecraft_pathfind::{
    MalusTable, MobFlags, PathInputs, PathQueries, PathScratch, PathTypeTable, PathTypeTableError,
    SearchRegion, StateFlags,
};
use mcrs_minecraft_physics::{PathType, TestWorld};
use mcrs_voxel::{BlockPos, BlockStateId};

#[test]
fn state_tables_bake_context_free_tops_and_preserve_supplied_flags() {
    for (state, top) in [
        (mcrs_voxel::BlockStateId::AIR, 0.0),
        (BLOCKS.stone, 1.0),
        (BLOCKS.bottom_slab, 0.5),
        (BLOCKS.fence, 1.5),
        (BLOCKS.scaffolding, 1.0),
        (BLOCKS.powder_snow, 0.0),
    ] {
        assert_eq!(TYPES.state(state).top, top);
    }
    let water = TYPES.state(BLOCKS.water);
    assert_eq!(water.kind, PathType::Water);
    assert_eq!(
        water.flags,
        StateFlags::FLOATABLE_FLUID.union(StateFlags::LAND_PATHFINDABLE)
    );
    assert_eq!(
        PathTypeTable::new(&BLOCKS.table, &[]).unwrap_err(),
        PathTypeTableError {
            collision: BLOCKS.table.len(),
            flags: 0,
        }
    );
}

#[test]
fn block_types_read_the_floor_and_the_first_danger_in_visit_order() {
    #[derive(Clone, Copy)]
    struct Case<'a> {
        floor: PathType,
        state: PathType,
        around: &'a [(BlockPos, PathType)],
        y: i32,
        expected: PathType,
    }
    let base = Case {
        floor: PathType::Blocked,
        state: PathType::Open,
        around: &[],
        y: 1,
        expected: PathType::Walkable,
    };
    let cases = [
        base,
        Case {
            around: &[(BlockPos::new(0, 2, 0), PathType::Fire)],
            ..base
        },
        Case {
            floor: PathType::Fire,
            expected: PathType::Fire,
            ..base
        },
        Case {
            floor: PathType::DropDown,
            expected: PathType::OnTopOfDropDown,
            ..base
        },
        Case {
            state: PathType::DropDown,
            expected: PathType::DropDown,
            ..base
        },
        Case {
            around: &[(BlockPos::new(-1, 1, 0), PathType::Water)],
            expected: PathType::WaterBorder,
            ..base
        },
        Case {
            around: &[
                (BlockPos::new(-1, 0, -1), PathType::Water),
                (BlockPos::new(-1, 0, 0), PathType::Fire),
            ],
            expected: PathType::WaterBorder,
            ..base
        },
        Case {
            around: &[
                (BlockPos::new(-1, 2, 1), PathType::Water),
                (BlockPos::new(0, 0, -1), PathType::Damaging),
            ],
            expected: PathType::WaterBorder,
            ..base
        },
        Case {
            around: &[
                (BlockPos::new(-1, 0, 1), PathType::Damaging),
                (BlockPos::new(-1, 1, -1), PathType::Water),
            ],
            expected: PathType::DamagingInNeighbor,
            ..base
        },
        Case {
            y: 0,
            floor: PathType::Fire,
            expected: PathType::Open,
            ..base
        },
    ];
    for case in cases {
        let at = BlockPos::new(0, case.y, 0);
        let mut world = fixture::floor(case.y - 1, SPECIAL.id(case.floor));
        world.set(at, SPECIAL.id(case.state));
        for &(at, kind) in case.around {
            world.set(at, SPECIAL.id(kind));
        }
        let mob = PathInputs::new(at, 0.6, 1.95);
        let region = SearchRegion::new(at, 16.0, 0..=16);
        let mut scratch = PathScratch::default();
        let mut queries =
            PathQueries::new(&world, &SPECIAL.types, &mob, &region, &mut scratch).expect("queries");
        assert_eq!(queries.block_path_type(at), case.expected);
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of mob path type cases")]
fn mob_types_apply_doors_rails_volume_costs_and_f32_cell_counts() {
    #[derive(Clone)]
    struct Case<'a> {
        width: f32,
        floor: PathType,
        state: PathType,
        extra: &'a [(BlockPos, PathType)],
        flags: MobFlags,
        mob_y: i32,
        malus: MalusTable,
        expected: PathType,
    }
    let base = Case {
        width: 0.6,
        floor: PathType::Blocked,
        state: PathType::Open,
        extra: &[],
        flags: MobFlags::default(),
        mob_y: 1,
        malus: MalusTable::default(),
        expected: PathType::Walkable,
    };
    let wide = Case {
        width: 1.2,
        extra: &[(BlockPos::new(1, 0, 0), PathType::Fire)],
        expected: PathType::BigMobsCloseToDanger,
        ..base.clone()
    };
    let cases = [
        Case {
            floor: PathType::Fire,
            expected: PathType::Fire,
            ..base.clone()
        },
        Case {
            state: PathType::DoorWoodClosed,
            expected: PathType::DoorWoodClosed,
            ..base.clone()
        },
        Case {
            state: PathType::DoorWoodClosed,
            flags: MobFlags::default().union(MobFlags::OPEN_DOORS),
            expected: PathType::WalkableDoor,
            ..base.clone()
        },
        Case {
            state: PathType::DoorWoodClosed,
            flags: MobFlags::OPEN_DOORS,
            expected: PathType::DoorWoodClosed,
            ..base.clone()
        },
        Case {
            state: PathType::DoorOpen,
            flags: MobFlags::default().without(MobFlags::PASS_DOORS),
            expected: PathType::Blocked,
            ..base.clone()
        },
        Case {
            state: PathType::DoorOpen,
            expected: PathType::DoorOpen,
            ..base.clone()
        },
        Case {
            state: PathType::Rail,
            expected: PathType::UnpassableRail,
            ..base.clone()
        },
        Case {
            state: PathType::Rail,
            extra: &[(BlockPos::new(3, 1, 0), PathType::Rail)],
            expected: PathType::Rail,
            ..base.clone()
        },
        Case {
            state: PathType::Rail,
            mob_y: 2,
            extra: &[(BlockPos::new(3, 1, 0), PathType::Rail)],
            expected: PathType::Rail,
            ..base.clone()
        },
        wide.clone(),
        Case {
            width: 1.0_f32.next_down(),
            ..wide.clone()
        },
        Case {
            width: 1.0_f32.next_down().next_down(),
            expected: PathType::FireInNeighbor,
            ..wide
        },
        Case {
            width: 1.2,
            state: PathType::Water,
            extra: &[(BlockPos::new(1, 1, 0), PathType::Fire)],
            malus: MalusTable::new([(PathType::Fire, 8.0)]),
            expected: PathType::Fire,
            ..base.clone()
        },
        Case {
            width: 1.2,
            state: PathType::PowderSnow,
            extra: &[(BlockPos::new(1, 1, 0), PathType::Fence)],
            expected: PathType::Fence,
            ..base.clone()
        },
        Case {
            width: 1.2,
            state: PathType::Leaves,
            extra: &[(BlockPos::new(1, 1, 0), PathType::PowderSnow)],
            expected: PathType::PowderSnow,
            ..base
        },
    ];
    for case in cases {
        let at = BlockPos::new(0, 1, 0);
        let mut world = fixture::floor(0, SPECIAL.id(case.floor));
        world.set(at, SPECIAL.id(case.state));
        for &(at, kind) in case.extra {
            world.set(at, SPECIAL.id(kind));
        }
        let mut mob = PathInputs::new(BlockPos::new(3, case.mob_y, 0), case.width, 1.95);
        mob.flags = case.flags;
        mob.malus = case.malus;
        let region = SearchRegion::new(at, 16.0, 0..=16);
        let mut scratch = PathScratch::default();
        let mut queries =
            PathQueries::new(&world, &SPECIAL.types, &mob, &region, &mut scratch).expect("queries");
        assert_eq!(queries.mob_path_type(at), case.expected);
    }
}

#[test]
fn region_reads_include_whole_chunks_and_treat_unloaded_or_missing_states_as_air() {
    struct Case {
        at: BlockPos,
        unloaded: bool,
        missing: bool,
        expected: PathType,
    }
    let cases = [
        Case {
            at: BlockPos::new(15, 1, 0),
            unloaded: false,
            missing: false,
            expected: PathType::Blocked,
        },
        Case {
            at: BlockPos::new(16, 1, 0),
            unloaded: false,
            missing: false,
            expected: PathType::Open,
        },
        Case {
            at: BlockPos::new(0, 1, 0),
            unloaded: false,
            missing: false,
            expected: PathType::Blocked,
        },
        Case {
            at: BlockPos::new(0, 1, 0),
            unloaded: true,
            missing: false,
            expected: PathType::Open,
        },
        Case {
            at: BlockPos::new(0, -1, 0),
            unloaded: false,
            missing: false,
            expected: PathType::Open,
        },
        Case {
            at: BlockPos::new(0, 5, 0),
            unloaded: false,
            missing: false,
            expected: PathType::Open,
        },
        Case {
            at: BlockPos::new(0, 1, 0),
            unloaded: false,
            missing: true,
            expected: PathType::Open,
        },
    ];
    let mob = PathInputs::new(BlockPos::new(0, 1, 0), 0.6, 1.95);
    let region = SearchRegion::new(mob.block_position, 0.0, 0..=4);
    let mut worlds = Vec::new();
    for case in &cases {
        let mut world = TestWorld::new();
        let state = if case.missing {
            BlockStateId::checked(u16::MAX, usize::from(u16::MAX) + 1)
                .expect("state outside the path type table")
        } else {
            BLOCKS.stone
        };
        world.set(case.at, state);
        if case.unloaded {
            world.unload_chunk(0, 0);
        }
        worlds.push(world);
    }
    let mut scratch = PathScratch::default();
    for (case, world) in cases.iter().zip(&worlds) {
        let mut queries =
            PathQueries::new(world, &TYPES, &mob, &region, &mut scratch).expect("queries");
        assert_eq!(queries.block_path_type(case.at), case.expected);
    }
}
