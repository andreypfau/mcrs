use glam::IVec3;
use mcrs_minecraft_falling_block::EntityDrops;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::LandingSite;
use mcrs_minecraft_falling_block::LevelEvent;
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
    let cases = [
        // FBK-LND-001: moving_piston comes first, then the break flag.
        (suspicious, site(piston, stone), LandingOutcome::Wait, None),
        (
            suspicious,
            on_stone,
            LandingOutcome::Break,
            Some(LevelEvent::BlockBreak),
        ),
        // FBK-LND-002.
        (sand, on_stone, place(sand, None), None),
        (sand, site(air, air), item, None),
        (
            sand,
            LandingSite {
                water_contact: WaterContact::Wet,
                ..site(air, air)
            },
            place(sand, None),
            None,
        ),
        (
            sand,
            LandingSite {
                replaceable: Replaceable::Occupied,
                ..on_stone
            },
            item,
            None,
        ),
        (
            sand,
            LandingSite {
                support: Support::Fails,
                ..on_stone
            },
            item,
            None,
        ),
        // FBK-LND-004: a water source waterlogs, water at or beside the cell makes concrete.
        (dry_slab, site(water, stone), place(wet_slab, None), None),
        (dry_slab, site(flowing, stone), place(dry_slab, None), None),
        (
            powder,
            site(water, stone),
            place(powder, Some(concrete)),
            None,
        ),
        (
            powder,
            site(flowing, stone),
            place(powder, Some(concrete)),
            None,
        ),
        (
            powder,
            LandingSite {
                water_beside: WaterBeside::Present,
                ..on_stone
            },
            place(powder, Some(concrete)),
            None,
        ),
        (powder, site(air, water), item, None),
        (powder, on_stone, place(powder, None), None),
        // FBK-LND-007.
        (
            anvil,
            on_stone,
            place(anvil, None),
            Some(LevelEvent::AnvilLand),
        ),
        (anvil, site(air, air), item, Some(LevelEvent::AnvilDestroy)),
    ];
    for (falling, site, outcome, event) in cases {
        let block = start(&TABLE, IVec3::ZERO, falling).falling_block;
        let landing = block.land(&TABLE, &site, EntityDrops::On, Silence::Audible);
        assert_eq!(landing.outcome, outcome, "{falling:?} {site:?}");
        assert_eq!(landing.event, event, "{falling:?} {site:?}");
    }
}

#[test]
fn fbk_lnd_005_006_007_the_game_rule_and_the_silence() {
    let air = state("minecraft:air", &[]);
    let stone = state("minecraft:stone", &[]);
    let anvil = state("minecraft:anvil", &[]);
    let suspicious = state("minecraft:suspicious_gravel", &[]);
    let cases = [
        (
            anvil,
            air,
            EntityDrops::Off,
            Silence::Audible,
            LandingOutcome::Item(Remains::Nothing),
            None,
        ),
        (
            anvil,
            air,
            EntityDrops::On,
            Silence::Silent,
            LandingOutcome::Item(Remains::Item),
            None,
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
            None,
        ),
        (
            suspicious,
            stone,
            EntityDrops::Off,
            Silence::Silent,
            LandingOutcome::Break,
            Some(LevelEvent::BlockBreak),
        ),
    ];
    for (falling, below, entity_drops, silence, outcome, event) in cases {
        let block = start(&TABLE, IVec3::ZERO, falling).falling_block;
        let landing = block.land(&TABLE, &site(air, below), entity_drops, silence);
        assert_eq!(
            landing.outcome, outcome,
            "{falling:?} {entity_drops:?} {silence:?}"
        );
        assert_eq!(
            landing.event, event,
            "{falling:?} {entity_drops:?} {silence:?}"
        );
    }
}
