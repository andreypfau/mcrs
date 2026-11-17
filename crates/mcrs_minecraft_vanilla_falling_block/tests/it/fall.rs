use std::num::NonZeroU32;

use bevy_ecs::world::World;
use glam::DVec3;
use glam::IVec3;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_falling_block::AtLanding;
use mcrs_minecraft_falling_block::Fall;
use mcrs_minecraft_falling_block::ItemDrop;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_falling_block::Start;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_minecraft_vanilla_falling_block::FallRequest;
use mcrs_minecraft_vanilla_falling_block::PendingFall;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::fixture::app;
use crate::fixture::block;
use crate::fixture::drop_block;
use crate::fixture::falling_blocks;
use crate::fixture::item_kinds;
use crate::fixture::item_of;
use crate::fixture::level_events;
use crate::fixture::run;
use crate::fixture::set_block;
use crate::fixture::set_velocity;
use crate::fixture::state;
use crate::fixture::state_with;

const START: BlockPos = BlockPos::new(0, 5, 0);
const FLOOR: BlockPos = BlockPos::new(0, 0, 0);

#[test]
fn fbk_spn_001_a_requested_fall_leaves_the_leftover_and_spawns_the_falling_block() {
    let mut app = app(Settings::default());
    let world = app.world_mut();
    let sand = state("sand");
    drop_block(world, START, sand);
    run(&mut app, 1);
    let world = app.world_mut();
    assert_eq!(block(world, START), state("air"));
    let falling = falling_blocks(world);
    let [(entity, falling)] = falling.as_slice() else {
        panic!("one falling block, found {falling:?}");
    };
    assert_eq!(falling.0.carried, sand);
    let look = &world.get::<Appearance>(*entity).unwrap().look;
    assert_eq!(
        *look,
        SpawnLook::FallingBlock {
            state: BlockStateNumber(sand.number()),
            start: START,
        }
    );
}

#[test]
fn fbk_lnd_the_landings_on_the_floor() {
    struct Case {
        name: &'static str,
        carried: &'static str,
        under: Option<&'static str>,
        placed: &'static str,
        item: Option<&'static str>,
        events: &'static [i32],
    }
    let cases = [
        Case {
            name: "FBK-LND-001 sand over air lands as sand",
            carried: "sand",
            under: None,
            placed: "sand",
            item: None,
            events: &[],
        },
        Case {
            name: "FBK-LND-006 a cell that is not replaceable takes the item",
            carried: "sand",
            under: Some("stone_slab"),
            placed: "stone_slab",
            item: Some("sand"),
            events: &[],
        },
        Case {
            name: "FBK-LND-004 concrete powder in water becomes concrete",
            carried: "white_concrete_powder",
            under: Some("water"),
            placed: "white_concrete",
            item: None,
            events: &[],
        },
        Case {
            name: "FBK-LND-005 the break flag makes no block and sends the event 2001",
            carried: "suspicious_sand",
            under: None,
            placed: "air",
            item: None,
            events: &[2001],
        },
    ];
    for case in cases {
        let mut app = app(Settings::default());
        let world = app.world_mut();
        if let Some(under) = case.under {
            set_block(world, FLOOR, state(under));
        }
        let carried = state(case.carried);
        drop_block(world, START, carried);
        run(&mut app, 60);
        let world = app.world_mut();
        assert_eq!(block(world, FLOOR), state(case.placed), "{}", case.name);
        assert!(falling_blocks(world).is_empty(), "{}", case.name);
        let items = item_kinds(world);
        let expected: Vec<_> = case.item.map(item_of).into_iter().collect();
        assert_eq!(items, expected, "{}", case.name);
        let events = level_events(world);
        let effects: Vec<i32> = events.iter().map(|event| event.effect).collect();
        assert_eq!(effects, case.events, "{}", case.name);
        for event in events {
            assert_eq!(event.pos, FLOOR, "{}", case.name);
            assert_eq!(
                event.effect_data,
                i32::from(carried.number()),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn fbk_the_water_of_a_concrete_powder() {
    struct Case {
        name: &'static str,
        start: BlockPos,
        velocity: Option<DVec3>,
        set: Vec<(BlockPos, BlockStateId)>,
        landed: BlockPos,
        placed: &'static str,
    }
    let waterlogged_top_slab =
        state_with("stone_slab", &[("type", "top"), ("waterlogged", "true")]);
    let flowing_water = state_with("water", &[("level", "1")]);
    let cases = [
        Case {
            name: "FBK-LND-004 a waterlogged top slab above the target does not count",
            start: FLOOR + IVec3::Y,
            velocity: None,
            set: vec![(FLOOR + IVec3::Y, waterlogged_top_slab)],
            landed: FLOOR,
            placed: "white_concrete_powder",
        },
        Case {
            name: "FBK-TCK-003 a fast segment finds a source 10 blocks back over a wet cell",
            start: BlockPos::new(0, 30, 0),
            velocity: Some(DVec3::new(0.0, -12.0, 0.0)),
            set: [16, 17, 18]
                .map(|y| (BlockPos::new(0, y, 0), flowing_water))
                .into_iter()
                .chain([(BlockPos::new(0, 20, 0), state("water"))])
                .collect(),
            landed: BlockPos::new(0, 20, 0),
            placed: "white_concrete",
        },
    ];
    for case in cases {
        let mut app = app(Settings::default());
        drop_block(app.world_mut(), case.start, state("white_concrete_powder"));
        run(&mut app, 1);
        let world = app.world_mut();
        let [(entity, _)] = falling_blocks(world)[..] else {
            panic!("{}: one falling block", case.name);
        };
        if let Some(velocity) = case.velocity {
            set_velocity(world, entity, velocity);
        }
        for (pos, state) in case.set {
            set_block(world, pos, state);
        }
        run(&mut app, 60);
        let world = app.world_mut();
        assert_eq!(
            block(world, case.landed),
            state(case.placed),
            "{}",
            case.name
        );
        assert!(falling_blocks(world).is_empty(), "{}", case.name);
    }
}

#[test]
fn fbk_tck_004_the_maximum_age_drops_the_item_and_0_disables_it() {
    let high = BlockPos::new(0, 40, 0);
    for (max_age, placed, items) in [(10, "air", vec![item_of("sand")]), (0, "sand", vec![])] {
        let mut app = app(Settings::new(NonZeroU32::new(max_age)));
        drop_block(app.world_mut(), high, state("sand"));
        run(&mut app, 120);
        let world = app.world_mut();
        assert_eq!(block(world, FLOOR), state(placed), "max age {max_age}");
        assert_eq!(item_kinds(world), items, "max age {max_age}");
        assert!(falling_blocks(world).is_empty(), "max age {max_age}");
    }
}

#[test]
fn one_falling_block_for_a_block_and_one_block_for_a_cell() {
    struct Case {
        name: &'static str,
        spawn: fn(&mut World),
        falling: usize,
        items: Vec<Kind>,
    }
    fn two_requests(world: &mut World) {
        drop_block(world, START, state("sand"));
        world.write_message(FallRequest {
            pos: START,
            state: state("sand"),
        });
    }
    fn two_starts(world: &mut World) {
        let start = Start {
            leftover: state("air"),
            position: DVec3::new(0.5, 5.0, 0.5),
            falling_block: Fall {
                carried: state("sand"),
                item_drop: ItemDrop::Allowed,
                at_landing: AtLanding::Settle,
                start: *START,
            },
        };
        world.spawn(PendingFall(start));
        world.spawn(PendingFall(start));
    }
    let cases = [
        Case {
            name: "FBK-SPN-001 two requests for one block make one falling block",
            spawn: two_requests,
            falling: 1,
            items: vec![],
        },
        Case {
            name: "FBK-LND-002 a cell that a landing of the same tick fills takes the item",
            spawn: two_starts,
            falling: 2,
            items: vec![item_of("sand")],
        },
    ];
    for case in cases {
        let mut app = app(Settings::default());
        (case.spawn)(app.world_mut());
        run(&mut app, 1);
        assert_eq!(
            falling_blocks(app.world_mut()).len(),
            case.falling,
            "{}",
            case.name
        );
        run(&mut app, 60);
        let world = app.world_mut();
        assert_eq!(block(world, FLOOR), state("sand"), "{}", case.name);
        assert_eq!(item_kinds(world), case.items, "{}", case.name);
        assert!(falling_blocks(world).is_empty(), "{}", case.name);
    }
}

#[test]
fn fbk_tck_001_a_falling_block_that_the_pass_does_not_step_takes_no_tick() {
    struct Case {
        name: &'static str,
        carried: &'static str,
        floor: &'static str,
        velocity: DVec3,
    }
    let cases = [
        Case {
            name: "FBK-TCK-004 the maximum age 5 does not end it",
            carried: "sand",
            floor: "air",
            velocity: DVec3::ZERO,
        },
        Case {
            name: "FBK-TCK-003 a concrete powder tests no segment of an earlier step",
            carried: "white_concrete_powder",
            floor: "water",
            velocity: DVec3::new(0.0, -12.0, 0.0),
        },
    ];
    for case in cases {
        let mut app = app(Settings::new(NonZeroU32::new(5)));
        let world = app.world_mut();
        set_block(world, FLOOR, state(case.floor));
        world.write_message(FallRequest {
            pos: BlockPos::new(100, 5, 0),
            state: state(case.carried),
        });
        run(&mut app, 1);
        let world = app.world_mut();
        let [(entity, _)] = falling_blocks(world)[..] else {
            panic!("{}: one falling block", case.name);
        };
        set_velocity(world, entity, case.velocity);
        run(&mut app, 20);
        let world = app.world_mut();
        assert_eq!(falling_blocks(world).len(), 1, "{}", case.name);
        assert!(item_kinds(world).is_empty(), "{}", case.name);
        assert_eq!(block(world, FLOOR), state(case.floor), "{}", case.name);
    }
}
