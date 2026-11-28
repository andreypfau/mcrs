use crate::fixture::TABLE;
use crate::fixture::state;

#[test]
fn fbk_trg_004_a_state_is_free_when_replaceable_fire_water_or_lava() {
    let cases = [
        ("minecraft:air", &[][..], true),
        ("minecraft:void_air", &[], true),
        ("minecraft:short_grass", &[], true),
        ("minecraft:snow", &[("layers", "1")], true),
        ("minecraft:snow", &[("layers", "8")], true),
        ("minecraft:fire", &[], true),
        ("minecraft:soul_fire", &[], true),
        ("minecraft:water", &[], true),
        ("minecraft:water", &[("level", "3")], true),
        ("minecraft:lava", &[], true),
        ("minecraft:stone", &[], false),
        ("minecraft:oak_slab", &[("waterlogged", "true")], false),
        ("minecraft:torch", &[], false),
    ];
    for (block, properties, free) in cases {
        assert_eq!(
            TABLE.is_free(state(block, properties)),
            free,
            "{block} {properties:?}"
        );
    }
}

#[test]
fn fbk_trg_001_the_default_gravity_blocks_fall_onto_a_free_state() {
    let air = state("minecraft:air", &[]);
    let stone = state("minecraft:stone", &[]);
    let cases = [
        ("minecraft:sand", true),
        ("minecraft:red_sand", true),
        ("minecraft:gravel", true),
        ("minecraft:white_concrete_powder", true),
        ("minecraft:black_concrete_powder", true),
        ("minecraft:anvil", true),
        ("minecraft:damaged_anvil", true),
        ("minecraft:suspicious_sand", true),
        ("minecraft:suspicious_gravel", true),
        ("minecraft:dragon_egg", false),
        ("minecraft:white_concrete", false),
        ("minecraft:stone", false),
    ];
    for (block, falls) in cases {
        let gravity = state(block, &[]);
        assert_eq!(TABLE.falls(gravity, air), falls, "{block} over air");
        assert!(!TABLE.falls(gravity, stone), "{block} over stone");
    }
}
