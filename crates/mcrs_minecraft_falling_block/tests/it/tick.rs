use std::num::NonZeroU32;

use glam::IVec3;
use mcrs_minecraft_falling_block::AfterMove;
use mcrs_minecraft_falling_block::AtLanding;
use mcrs_minecraft_falling_block::EntityDrops;
use mcrs_minecraft_falling_block::Fall;
use mcrs_minecraft_falling_block::Ground;
use mcrs_minecraft_falling_block::ItemDrop;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_falling_block::TickStart;
use mcrs_minecraft_falling_block::WaterContact;
use mcrs_minecraft_falling_block::WaterSweep;
use mcrs_minecraft_falling_block::WorldHeight;
use mcrs_voxel::BlockStateId;

const HEIGHT: WorldHeight = WorldHeight {
    min_y: -64,
    height: 384,
};

fn falling(item_drop: ItemDrop) -> Fall {
    Fall {
        carried: BlockStateId::checked(118, 200).unwrap(),
        item_drop,
        at_landing: AtLanding::Settle,
        start: IVec3::ZERO,
    }
}

#[test]
fn fbk_tck_001_a_falling_block_of_air_ends_before_it_ages() {
    let air = Fall {
        carried: BlockStateId::AIR,
        ..falling(ItemDrop::Allowed)
    };
    assert_eq!(air.begin_tick(), TickStart::Remove);
    assert_eq!(falling(ItemDrop::Allowed).begin_tick(), TickStart::Move);
}

#[test]
fn fbk_tck_004_005_the_time_limits_after_the_move() {
    let default = Settings::default();
    let disabled = Settings::new(None);
    let five = Settings::new(NonZeroU32::new(5));
    let airborne = (Ground::Airborne, WaterContact::Dry);
    let item = AfterMove::Expire(Remains::Item);
    let cases = [
        (599, default, airborne, 64, AfterMove::Fall),
        (600, default, airborne, 64, item),
        (4, five, airborne, 64, AfterMove::Fall),
        (5, five, airborne, 64, item),
        (2_147_483_647, disabled, airborne, 64, AfterMove::Fall),
        (100, disabled, airborne, -100, item),
        (99, disabled, airborne, -100, AfterMove::Fall),
        (100, disabled, airborne, -64, item),
        (100, disabled, airborne, -63, AfterMove::Fall),
        (100, disabled, airborne, 319, AfterMove::Fall),
        (100, disabled, airborne, 320, item),
        (
            700,
            default,
            (Ground::OnGround, WaterContact::Dry),
            -100,
            AfterMove::Land,
        ),
        (
            700,
            default,
            (Ground::Airborne, WaterContact::Wet),
            -100,
            AfterMove::Land,
        ),
    ];
    for (age_before, settings, (ground, water), cell_y, expected) in cases {
        let block = falling(ItemDrop::Allowed);
        assert_eq!(block.begin_tick(), TickStart::Move);
        let age = age_before + 1;
        let decision = block.after_move(
            age,
            settings,
            ground,
            water,
            cell_y,
            HEIGHT,
            EntityDrops::On,
        );
        assert_eq!(
            decision, expected,
            "age {age_before}, {settings:?}, {ground:?}, {water:?}, y {cell_y}"
        );
    }
}

#[test]
fn fbk_tck_004_an_expired_block_drops_only_with_the_drop_flag_and_the_game_rule() {
    let cases = [
        (ItemDrop::Allowed, EntityDrops::On, Remains::Item),
        (ItemDrop::Allowed, EntityDrops::Off, Remains::Nothing),
        (ItemDrop::Cancelled, EntityDrops::On, Remains::Nothing),
    ];
    for (item_drop, entity_drops, remains) in cases {
        let decision = falling(item_drop).after_move(
            601,
            Settings::default(),
            Ground::Airborne,
            WaterContact::Dry,
            64,
            HEIGHT,
            entity_drops,
        );
        assert_eq!(
            decision,
            AfterMove::Expire(remains),
            "{item_drop:?} {entity_drops:?}"
        );
    }
}

#[test]
fn fbk_tck_003_a_square_speed_above_1_tests_the_segment() {
    let cases = [
        (glam::DVec3::new(0.0, -1.0, 0.0), WaterSweep::Cell),
        (glam::DVec3::new(0.0, -1.01, 0.0), WaterSweep::Segment),
        (glam::DVec3::new(0.8, -0.8, 0.0), WaterSweep::Segment),
    ];
    for (velocity, sweep) in cases {
        assert_eq!(WaterSweep::of(velocity), sweep, "{velocity}");
    }
}
