use glam::DVec3;
use glam::IVec3;
use mcrs_minecraft_falling_block::AtLanding;
use mcrs_minecraft_falling_block::ItemDrop;
use mcrs_minecraft_falling_block::start;

use crate::fixture::TABLE;
use crate::fixture::state;

#[test]
fn fbk_spn_001_a_fall_leaves_the_fluid_and_carries_the_dry_state() {
    let block = IVec3::new(3, -64, -2);
    let air = state("minecraft:air", &[]);
    let water = state("minecraft:water", &[]);
    let wet_slab = state("minecraft:oak_slab", &[("waterlogged", "true")]);
    let dry_slab = state("minecraft:oak_slab", &[("waterlogged", "false")]);
    let sand = state("minecraft:sand", &[]);
    let suspicious = state("minecraft:suspicious_gravel", &[]);
    let cases = [
        (sand, air, sand, AtLanding::Settle),
        (suspicious, air, suspicious, AtLanding::Break),
        (wet_slab, water, dry_slab, AtLanding::Settle),
        (dry_slab, air, dry_slab, AtLanding::Settle),
    ];
    for (falling, leftover, carried, at_landing) in cases {
        let start = start(&TABLE, block, falling);
        assert_eq!(start.leftover, leftover, "{falling:?}");
        assert_eq!(start.position, DVec3::new(3.5, -64.0, -1.5));
        let falling_block = start.falling_block;
        assert_eq!(falling_block.carried, carried, "{falling:?}");
        assert_eq!(falling_block.at_landing, at_landing, "{falling:?}");
        assert_eq!(falling_block.item_drop, ItemDrop::Allowed);
        assert_eq!(falling_block.start, block);
    }
}
