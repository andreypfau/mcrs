use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_voxel::BlockPos;

use crate::fixture::app;
use crate::fixture::block;
use crate::fixture::drop_block;
use crate::fixture::falling_blocks;
use crate::fixture::item_kinds;
use crate::fixture::item_of;
use crate::fixture::level_events;
use crate::fixture::run;
use crate::fixture::set_block;
use crate::fixture::state;

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

/// The landing of a falling block that starts at [`START`] over the block `under` at [`FLOOR`]:
/// the block at the floor after it, the item it drops and the level events of the landing.
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

/// A falling block that the floor 40 blocks below does not reach in `max_age` ticks.
#[test]
fn fbk_tck_004_the_maximum_age_drops_the_item_and_0_disables_it() {
    let high = BlockPos::new(0, 40, 0);
    for (max_age, placed, items) in [(10, "air", vec![item_of("sand")]), (0, "sand", vec![])] {
        let mut app = app(Settings { max_age });
        drop_block(app.world_mut(), high, state("sand"));
        run(&mut app, 120);
        let world = app.world_mut();
        assert_eq!(block(world, FLOOR), state(placed), "max age {max_age}");
        assert_eq!(item_kinds(world), items, "max age {max_age}");
        assert!(falling_blocks(world).is_empty(), "max age {max_age}");
    }
}
