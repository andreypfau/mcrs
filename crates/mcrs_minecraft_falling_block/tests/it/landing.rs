use glam::IVec3;
use mcrs_minecraft_falling_block::AtLanding;
use mcrs_minecraft_falling_block::EntityDrops;
use mcrs_minecraft_falling_block::Fall;
use mcrs_minecraft_falling_block::LandingEvent;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::LandingSite;
use mcrs_minecraft_falling_block::Placement;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_falling_block::Replaceable;
use mcrs_minecraft_falling_block::Silence;
use mcrs_minecraft_falling_block::Support;
use mcrs_minecraft_falling_block::WaterBeside;
use mcrs_minecraft_falling_block::WaterContact;
use mcrs_minecraft_falling_block::start;
use mcrs_voxel::BlockStateId;

use crate::fixture::TABLE;
use crate::fixture::state;

fn site(target: BlockStateId, below: BlockStateId) -> LandingSite {
    LandingSite {
        target,
        below,
        replaceable: Replaceable::Replaceable,
        support: Support::Stays,
        water_contact: WaterContact::Dry,
        water_beside: WaterBeside::Absent,
    }
}

fn place(placed: BlockStateId, concrete: Option<BlockStateId>) -> LandingOutcome {
    LandingOutcome::Place(Placement { placed, concrete })
}

#[test]
fn fbk_lnd_001_to_007_the_result_of_a_landing() {
    let air = state("minecraft:air", &[]);
    let stone = state("minecraft:stone", &[]);
    let water = state("minecraft:water", &[]);
    let flowing = state("minecraft:water", &[("level", "3")]);
    let piston = state("minecraft:moving_piston", &[]);
    let sand = state("minecraft:sand", &[]);
    let suspicious = state("minecraft:suspicious_sand", &[]);
    let anvil = state("minecraft:anvil", &[]);
    let powder = state("minecraft:white_concrete_powder", &[]);
    let concrete = state("minecraft:white_concrete", &[]);
    let dry_slab = state("minecraft:oak_slab", &[("waterlogged", "false")]);
    let wet_slab = state("minecraft:oak_slab", &[("waterlogged", "true")]);
    let on_stone = site(air, stone);
    let item = LandingOutcome::Item(Remains::Item);
    let cases: &[(BlockStateId, LandingSite, LandingOutcome, &[LandingEvent])] = &[
        (suspicious, site(piston, stone), LandingOutcome::Wait, &[]),
        (
            suspicious,
            on_stone,
            LandingOutcome::Break,
            &[LandingEvent::BlockBreak][..],
        ),
        (sand, on_stone, place(sand, None), &[]),
        (sand, site(air, air), item, &[]),
        (
            sand,
            LandingSite {
                water_contact: WaterContact::Wet,
                ..site(air, air)
            },
            place(sand, None),
            &[],
        ),
        (
            sand,
            LandingSite {
                replaceable: Replaceable::Occupied,
                ..on_stone
            },
            item,
            &[],
        ),
        (
            sand,
            LandingSite {
                support: Support::Fails,
                ..on_stone
            },
            item,
            &[],
        ),
        (dry_slab, site(water, stone), place(wet_slab, None), &[]),
        (dry_slab, site(flowing, stone), place(dry_slab, None), &[]),
        (
            powder,
            site(water, stone),
            place(powder, Some(concrete)),
            &[],
        ),
        (
            powder,
            site(flowing, stone),
            place(powder, Some(concrete)),
            &[],
        ),
        (
            powder,
            LandingSite {
                water_beside: WaterBeside::Present,
                ..on_stone
            },
            place(powder, Some(concrete)),
            &[],
        ),
        (powder, site(air, water), item, &[]),
        (powder, on_stone, place(powder, None), &[]),
        (
            anvil,
            on_stone,
            place(anvil, None),
            &[LandingEvent::AnvilLand][..],
        ),
        (
            anvil,
            site(air, air),
            item,
            &[LandingEvent::AnvilDestroy][..],
        ),
    ];
    for &(falling, site, outcome, event) in cases {
        let block = start(&TABLE, IVec3::ZERO, falling).falling_block;
        let landing = block.land(&TABLE, &site, EntityDrops::On, Silence::Audible);
        assert_eq!(landing.outcome, outcome, "{falling:?} {site:?}");
        assert_eq!(
            landing.events().collect::<Vec<_>>(),
            event,
            "{falling:?} {site:?}"
        );
    }
}

#[test]
fn fbk_lnd_005_006_007_the_game_rule_and_the_silence() {
    let air = state("minecraft:air", &[]);
    let stone = state("minecraft:stone", &[]);
    let anvil = state("minecraft:anvil", &[]);
    let suspicious = state("minecraft:suspicious_gravel", &[]);
    let cases: &[(
        BlockStateId,
        BlockStateId,
        EntityDrops,
        Silence,
        LandingOutcome,
        &[LandingEvent],
    )] = &[
        (
            anvil,
            air,
            EntityDrops::Off,
            Silence::Audible,
            LandingOutcome::Item(Remains::Nothing),
            &[],
        ),
        (
            anvil,
            air,
            EntityDrops::On,
            Silence::Silent,
            LandingOutcome::Item(Remains::Item),
            &[],
        ),
        (
            anvil,
            stone,
            EntityDrops::On,
            Silence::Silent,
            LandingOutcome::Place(Placement {
                placed: anvil,
                concrete: None,
            }),
            &[],
        ),
        (
            suspicious,
            stone,
            EntityDrops::Off,
            Silence::Silent,
            LandingOutcome::Break,
            &[LandingEvent::BlockBreak][..],
        ),
    ];
    for &(falling, below, entity_drops, silence, outcome, event) in cases {
        let block = start(&TABLE, IVec3::ZERO, falling).falling_block;
        let landing = block.land(&TABLE, &site(air, below), entity_drops, silence);
        assert_eq!(
            landing.outcome, outcome,
            "{falling:?} {entity_drops:?} {silence:?}"
        );
        assert_eq!(
            landing.events().collect::<Vec<_>>(),
            event,
            "{falling:?} {entity_drops:?} {silence:?}"
        );
    }
}

#[test]
fn fbk_lnd_005_each_block_with_the_break_flag_sends_the_break_event() {
    let air = state("minecraft:air", &[]);
    let stone = state("minecraft:stone", &[]);
    let anvil = state("minecraft:anvil", &[]);
    let cases: &[(BlockStateId, Silence, &[LandingEvent])] = &[
        (
            state("minecraft:sand", &[]),
            Silence::Audible,
            &[LandingEvent::BlockBreak],
        ),
        (
            anvil,
            Silence::Audible,
            &[LandingEvent::BlockBreak, LandingEvent::AnvilDestroy],
        ),
        (anvil, Silence::Silent, &[LandingEvent::BlockBreak]),
    ];
    for &(falling, silence, events) in cases {
        let block = Fall {
            at_landing: AtLanding::Break,
            ..start(&TABLE, IVec3::ZERO, falling).falling_block
        };
        let landing = block.land(&TABLE, &site(air, stone), EntityDrops::Off, silence);
        assert_eq!(landing.outcome, LandingOutcome::Break, "{falling:?}");
        assert_eq!(
            landing.events().collect::<Vec<_>>(),
            events,
            "{falling:?} {silence:?}"
        );
    }
}
