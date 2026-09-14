use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};

use crate::block_entity::GeneratedBlockEntity;
use crate::feature::holds;

#[derive(Clone, Debug)]
pub struct CompiledMonsterRoom {
    pub cobblestone: VoxelId,
    pub mossy_cobblestone: VoxelId,
    pub spawner: VoxelId,
    /// The chest in each horizontal facing, indexed like `Direction::HORIZONTAL`.
    /// A default chest faces north, which is index 0.
    pub chest_facing: [VoxelId; 4],
    pub chest_states: StateMask,
    pub spawner_states: StateMask,
    /// `BlockTags.FEATURES_CANNOT_REPLACE`, the one predicate every write of
    /// this feature is guarded by.
    pub cannot_replace: StateMask,
}

const DUNGEON_LOOT: &str = "minecraft:chests/simple_dungeon";
const BONUS_CHEST_LOOT: &str = "minecraft:chests/spawn_bonus_chest";

/// `MonsterRoomFeature.MOBS`, whose repeat of the zombie is the weighting.
const MOBS: [&str; 4] = [
    "minecraft:skeleton",
    "minecraft:zombie",
    "minecraft:zombie",
    "minecraft:spider",
];

#[derive(Clone, Debug)]
pub struct CompiledBonusChest {
    pub chest: VoxelId,
    pub torch: VoxelId,
}
pub fn place_monster_room<W: WorldGenVolume>(
    config: &CompiledMonsterRoom,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    origin: BlockPos,
) -> bool {
    let x_radius = rng.next_i32_bound(2) + 2;
    let (min_x, max_x) = (-x_radius - 1, x_radius + 1);
    let z_radius = rng.next_i32_bound(2) + 2;
    let (min_z, max_z) = (-z_radius - 1, z_radius + 1);

    let mut holes = 0;
    for dx in min_x..=max_x {
        for dy in -1..=4 {
            for dz in min_z..=max_z {
                let pos = origin + IVec3::new(dx, dy, dz);
                let solid = volume.holds(&volume.world().solid, pos);
                if (dy == -1 || dy == 4) && !solid {
                    return false;
                }
                if (dx == min_x || dx == max_x || dz == min_z || dz == max_z)
                    && dy == 0
                    && volume.is_air(pos)
                    && volume.is_air(pos + IVec3::Y)
                {
                    holes += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&holes) {
        return false;
    }

    let min_y = volume.extent().min_y;
    for dx in min_x..=max_x {
        for dy in (-1..=3).rev() {
            for dz in min_z..=max_z {
                let pos = origin + IVec3::new(dx, dy, dz);
                let state = volume.get(pos);
                // `dy == 4` is unreachable in this loop and is kept because the
                // reference tests it; dropping it would change nothing but the
                // next reader's confidence.
                if dx == min_x || dy == -1 || dz == min_z || dx == max_x || dy == 4 || dz == max_z {
                    if pos.y >= min_y && !volume.holds(&volume.world().solid, pos + IVec3::NEG_Y) {
                        volume.set(pos, volume.world().cave_air);
                    } else if holds(&volume.world().solid, state)
                        && !holds(&config.chest_states, state)
                    {
                        let state = if dy == -1 && rng.next_i32_bound(4) != 0 {
                            config.mossy_cobblestone
                        } else {
                            config.cobblestone
                        };
                        volume.set_unless(&config.cannot_replace, pos, state);
                    }
                } else if !holds(&config.chest_states, state)
                    && !holds(&config.spawner_states, state)
                {
                    volume.set_unless(&config.cannot_replace, pos, volume.world().cave_air);
                }
            }
        }
    }

    for _ in 0..2 {
        for _ in 0..3 {
            let pos = BlockPos::new(
                origin.x + rng.next_i32_bound(x_radius * 2 + 1) - x_radius,
                origin.y,
                origin.z + rng.next_i32_bound(z_radius * 2 + 1) - z_radius,
            );
            if !volume.is_air(pos) {
                continue;
            }
            let walls = Direction::HORIZONTAL
                .iter()
                .filter(|side| volume.holds(&volume.world().solid, pos + side.normal()))
                .count();
            if walls != 1 {
                continue;
            }
            let chest = reorient(config, volume, pos);
            if volume.set_unless(&config.cannot_replace, pos, chest) {
                entities.push(GeneratedBlockEntity::chest(
                    pos,
                    DUNGEON_LOOT.to_owned(),
                    rng.next_java_long(),
                ));
            }
            break;
        }
    }

    let placed = volume.set_unless(&config.cannot_replace, origin, config.spawner);
    if placed || volume.holds(&config.spawner_states, origin) {
        let mob = MOBS[rng.next_i32_bound(4) as usize];
        entities.push(GeneratedBlockEntity::mob_spawner(origin, mob));
    }
    true
}

/// `StructurePiece.reorient` for a chest: back onto the one solid neighbour, or
/// walk the default facing round until it points at open space.
fn reorient<W: WorldGenVolume>(config: &CompiledMonsterRoom, volume: &W, pos: BlockPos) -> VoxelId {
    let mut only_solid = None;
    for (index, side) in Direction::HORIZONTAL.iter().enumerate() {
        let state = volume.get(pos + side.normal());
        if holds(&config.chest_states, state) {
            return config.chest_facing[0];
        }
        if holds(&volume.world().solid_render, state) {
            if only_solid.is_some() {
                only_solid = None;
                break;
            }
            only_solid = Some(index);
        }
    }
    if let Some(index) = only_solid {
        return config.chest_facing[(index + 2) % 4];
    }
    let solid_render = |facing: usize| {
        let step = Direction::HORIZONTAL[facing].normal();
        volume.holds(&volume.world().solid_render, pos + step)
    };
    let mut facing = 0;
    if solid_render(facing) {
        facing = (facing + 2) % 4;
    }
    if solid_render(facing) {
        facing = (facing + 1) % 4;
    }
    if solid_render(facing) {
        facing = (facing + 2) % 4;
    }
    config.chest_facing[facing]
}

/// `Util.toShuffledList(IntStream.rangeClosed(min, max), random)`.
fn shuffled_range(rng: &mut XoroshiroRandom, min: i32, max: i32) -> Vec<i32> {
    let mut values: Vec<i32> = (min..=max).collect();
    mcrs_minecraft_random::shuffle(&mut values, rng);
    values
}

/// `BonusChestFeature.place`: both shuffles are spent up front, so the column
/// the chest lands in costs no further draw.
pub fn place_bonus_chest<W: WorldGenVolume>(
    config: &CompiledBonusChest,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    origin: BlockPos,
) -> bool {
    let min_x = (origin.x >> 4) << 4;
    let min_z = (origin.z >> 4) << 4;
    let xs = shuffled_range(rng, min_x, min_x + 15);
    let zs = shuffled_range(rng, min_z, min_z + 15);

    for x in xs {
        for &z in &zs {
            let y = volume.height(HeightmapName::MotionBlockingNoLeaves, x, z);
            let pos = BlockPos::new(x, y, z);
            let state = volume.get(pos);
            if !holds(&volume.world().air_states, state)
                && !holds(&volume.world().empty_collision, state)
            {
                continue;
            }
            volume.set(pos, config.chest);
            entities.push(GeneratedBlockEntity::chest(
                pos,
                BONUS_CHEST_LOOT.to_owned(),
                rng.next_java_long(),
            ));
            for side in Direction::HORIZONTAL {
                let torch = pos + side.normal();
                // `canSupportCenter(level, below, UP)` as the full-collision-cube
                // flag; upgrade when the freeze can answer a per-face sturdiness.
                if volume.holds(&volume.world().sturdy_up, torch + IVec3::NEG_Y) {
                    volume.set(torch, config.torch);
                }
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const STONE: VoxelId = VoxelId(1);
    const COBBLE: VoxelId = VoxelId(2);
    const MOSSY: VoxelId = VoxelId(3);
    const SPAWNER: VoxelId = VoxelId(4);
    const CAVE_AIR: VoxelId = VoxelId(5);
    const TORCH: VoxelId = VoxelId(6);
    const CHEST_NORTH: VoxelId = VoxelId(10);

    fn room() -> CompiledMonsterRoom {
        CompiledMonsterRoom {
            cobblestone: COBBLE,
            mossy_cobblestone: MOSSY,
            spawner: SPAWNER,
            chest_facing: [CHEST_NORTH, VoxelId(11), VoxelId(12), VoxelId(13)],
            chest_states: mask_of([CHEST_NORTH, VoxelId(11), VoxelId(12), VoxelId(13)]),
            spawner_states: mask_of([SPAWNER]),
            cannot_replace: StateMask::default(),
        }
    }

    /// Stone, cobble and moss are the solids; both airs are air.
    fn cave() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.cave_air = CAVE_AIR;
        volume.world.air_states = mask_of([AIR, CAVE_AIR]);
        volume.world.solid = mask_of([STONE, COBBLE, MOSSY]);
        volume.world.solid_render = mask_of([STONE, COBBLE, MOSSY]);
        volume.world.sturdy_up = mask_of([STONE]);
        volume
    }

    const ORIGIN: BlockPos = BlockPos::new(0, 20, 0);

    /// Solid stone everywhere but a single air gap on the room's edge at
    /// `dy == 0`, which is the one hole the reference needs to accept.
    fn stone_with_one_hole() -> FakeVolume {
        let mut volume = cave();
        for x in -12..=12 {
            for z in -12..=12 {
                for y in 0..=40 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        // Whichever of the two radii the source draws, one of these columns
        // sits on the room's own edge and is the single hole it needs.
        for dy in 0..=1 {
            volume.blocks.insert((0, ORIGIN.y + dy, -3), AIR);
            volume.blocks.insert((0, ORIGIN.y + dy, -4), AIR);
        }
        volume
    }

    #[test]
    fn a_sealed_box_has_no_hole_and_places_nothing() {
        let mut volume = cave();
        for x in -12..=12 {
            for z in -12..=12 {
                for y in 0..=40 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(7);
        assert!(!place_monster_room(
            &room(),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));
        assert!(entities.is_empty());
    }

    /// The hole scan runs after the two radius draws and before anything else,
    /// so a rejected room costs exactly those two.
    #[test]
    fn a_rejected_room_spends_two_draws() {
        let mut volume = cave();
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(7);
        let mut replay = rng.clone();
        place_monster_room(&room(), &mut volume, &mut rng, &mut entities, ORIGIN);
        replay.next_i32_bound(2);
        replay.next_i32_bound(2);
        assert_eq!(rng, replay);
    }

    #[test]
    fn a_room_leaves_a_spawner_and_its_mob() {
        let mut volume = stone_with_one_hole();
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(0x5eed_2024);
        assert!(place_monster_room(
            &room(),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));
        assert_eq!(volume.get(ORIGIN), SPAWNER);
        let spawner = entities
            .iter()
            .find(|entity| matches!(entity, GeneratedBlockEntity::MobSpawner { .. }))
            .expect("a monster room always leaves a spawner");
        assert_eq!(spawner.position(), ORIGIN);
    }

    /// The wall ring is cobble, mossy below the floor, and the interior is
    /// carved to cave air.
    #[test]
    fn the_room_is_carved_and_walled() {
        let mut volume = stone_with_one_hole();
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(0x5eed_2024);
        place_monster_room(&room(), &mut volume, &mut rng, &mut entities, ORIGIN);
        assert_eq!(volume.get(BlockPos::new(0, ORIGIN.y + 1, 0)), CAVE_AIR);
        let floor = volume.get(BlockPos::new(0, ORIGIN.y - 1, 0));
        assert!(floor == COBBLE || floor == MOSSY, "floor was {floor:?}");
    }

    /// Every draw the whole feature spends: two radii, one per floor cell of
    /// the wall ring, two per chest attempt plus a long for each chest that
    /// lands, and one for the mob.
    ///
    /// Self-recorded: the ladder is read off `MonsterRoomFeature`, the number
    /// off this implementation.
    #[test]
    fn room_draw_count_anchor() {
        let mut volume = stone_with_one_hole();
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(0x5eed_2024);
        let mut replay = rng.clone();
        place_monster_room(&room(), &mut volume, &mut rng, &mut entities, ORIGIN);

        replay.next_i32_bound(2);
        replay.next_i32_bound(2);
        assert_ne!(rng, replay, "the room draws well past its two radii");
        assert_eq!(rng.next_java_long(), ROOM_PIN);
    }

    const ROOM_PIN: i64 = -5825020126323992744;

    fn bonus() -> CompiledBonusChest {
        CompiledBonusChest {
            chest: CHEST_NORTH,
            torch: TORCH,
        }
    }

    #[test]
    fn bonus_chest_spends_two_shuffles_and_one_long() {
        let mut volume = cave();
        for x in -2..18 {
            for z in -2..18 {
                volume.heights.insert((x, z), 64);
                volume.blocks.insert((x, 63, z), STONE);
            }
        }
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(99);
        let mut replay = rng.clone();
        assert!(place_bonus_chest(
            &bonus(),
            &mut volume,
            &mut rng,
            &mut entities,
            BlockPos::new(8, 64, 8)
        ));
        for _ in 0..2 {
            for i in (2..=16).rev() {
                replay.next_i32_bound(i);
            }
        }
        replay.next_java_long();
        assert_eq!(rng, replay, "bonus chest draw sequence");
        assert_eq!(entities.len(), 1);
        let pos = entities[0].position();
        assert_eq!(volume.get(pos), CHEST_NORTH);
        for side in Direction::HORIZONTAL {
            let torch = pos + side.normal();
            assert_eq!(volume.get(torch), TORCH);
        }
    }

    #[test]
    fn bonus_chest_finds_nothing_in_a_solid_column() {
        let mut volume = cave();
        for x in 0..16 {
            for z in 0..16 {
                volume.heights.insert((x, z), 64);
                volume.blocks.insert((x, 64, z), STONE);
            }
        }
        let mut entities = Vec::new();
        let mut rng = XoroshiroRandom::new(99);
        assert!(!place_bonus_chest(
            &bonus(),
            &mut volume,
            &mut rng,
            &mut entities,
            BlockPos::new(8, 64, 8)
        ));
        assert!(entities.is_empty());
    }
}
