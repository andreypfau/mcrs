use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, StateMask, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

const HUGE_PROBABILITY: f32 = 0.06;

#[derive(Clone, Debug)]
pub struct CompiledHugeFungus {
    /// Every state of the block the nylium check names.
    pub valid_base: StateMask,
    pub stem_state: VoxelId,
    pub hat_state: VoxelId,
    /// Every state of the hat's block: what the skirt looks for below itself.
    pub hat_block: StateMask,
    pub decor_state: VoxelId,
    pub replaceable_blocks: Predicate,
    pub planted: bool,
    /// The hat is `nether_wart_block`, the only hat that hangs vines.
    pub place_vines: bool,
    pub weeping_vines_plant: VoxelId,
    /// `weeping_vines` at ages 23, 24 and 25.
    pub weeping_vines_by_age: [VoxelId; 3],
}

/// A huge crimson or warped fungus: a stem of one or nine columns under a hat
/// of stacked squares, with weeping vines hung from a nether wart hat.
pub fn place_huge_fungus<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    if !volume.holds(&cfg.valid_base, at - IVec3::Y) {
        return false;
    }

    let mut total_height = rng.next_i32_bound(10) + 4;
    if rng.next_i32_bound(12) == 0 {
        total_height *= 2;
    }
    // The reference measures the roof against the generator's height rather
    // than the dimension's top, so a world whose bottom is not zero is judged
    // against a ceiling that many blocks too low. Copied as it stands.
    if !cfg.planted && at.y + total_height + 1 >= volume.extent().depth {
        return false;
    }

    let huge = !cfg.planted && rng.next_f32() < HUGE_PROBABILITY;
    volume.set(at, volume.world().air);
    place_stem(cfg, volume, rng, at, total_height, huge);
    place_hat(cfg, volume, rng, at, total_height, huge);
    true
}

fn place_stem<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
    total_height: i32,
    huge: bool,
) {
    let radius = i32::from(huge);
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let corner = huge && dx.abs() == radius && dz.abs() == radius;
            for dy in 0..total_height {
                let pos = at + IVec3::new(dx, dy, dz);
                if !replaceable(cfg, volume, pos, true) {
                    continue;
                }
                if cfg.planted {
                    clear_below(volume, pos);
                    volume.set(pos, cfg.stem_state);
                } else if corner {
                    if rng.next_f32() < 0.1 {
                        volume.set(pos, cfg.stem_state);
                    }
                } else {
                    volume.set(pos, cfg.stem_state);
                }
            }
        }
    }
}

fn place_hat<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
    total_height: i32,
    huge: bool,
) {
    let hat_height = (rng.next_i32_bound(1 + total_height / 3) + 5).min(total_height);
    let hat_start = total_height - hat_height;
    let vines = |probability: f32| if cfg.place_vines { probability } else { 0.0 };

    for dy in hat_start..=total_height {
        // Drawn for every layer, whether or not it decides the radius.
        let mut radius = if dy < total_height - rng.next_i32_bound(3) {
            2
        } else {
            1
        };
        if hat_height > 8 && dy < hat_start + 4 {
            radius = 3;
        }
        if huge {
            radius += 1;
        }

        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let edge_x = dx == -radius || dx == radius;
                let edge_z = dz == -radius || dz == radius;
                let inside = !edge_x && !edge_z && dy != total_height;
                let corner = edge_x && edge_z;
                let skirt = dy < hat_start + 3;
                let pos = at + IVec3::new(dx, dy, dz);
                if !replaceable(cfg, volume, pos, false) {
                    continue;
                }
                if cfg.planted {
                    clear_below(volume, pos);
                }
                if skirt {
                    if !inside {
                        place_skirt_block(cfg, volume, rng, pos);
                    }
                } else if inside {
                    place_hat_block(cfg, volume, rng, pos, 0.1, 0.2, vines(0.1));
                } else if corner {
                    place_hat_block(cfg, volume, rng, pos, 0.01, 0.7, vines(0.083));
                } else {
                    place_hat_block(cfg, volume, rng, pos, 5.0e-4, 0.98, vines(0.07));
                }
            }
        }
    }
}

fn place_hat_block<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    pos: BlockPos,
    decor_probability: f32,
    hat_probability: f32,
    vines_probability: f32,
) {
    if rng.next_f32() < decor_probability {
        volume.set(pos, cfg.decor_state);
    } else if rng.next_f32() < hat_probability {
        volume.set(pos, cfg.hat_state);
        // Drawn even for a hat that hangs nothing, so the two hats keep one
        // draw sequence.
        if rng.next_f32() < vines_probability {
            hang_vines(cfg, volume, rng, pos);
        }
    }
}

fn place_skirt_block<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    pos: BlockPos,
) {
    if volume.holds(&cfg.hat_block, pos - IVec3::Y) {
        volume.set(pos, cfg.hat_state);
    } else if rng.next_f32() < 0.15 {
        volume.set(pos, cfg.hat_state);
        if cfg.place_vines && rng.next_i32_bound(11) == 0 {
            hang_vines(cfg, volume, rng, pos);
        }
    }
}

fn hang_vines<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    hat: BlockPos,
) {
    let mut pos = hat - IVec3::Y;
    if !volume.is_air(pos) {
        return;
    }
    let mut goal = rng.next_i32_bound(5) + 1;
    if rng.next_i32_bound(7) == 0 {
        goal *= 2;
    }
    for step in 0..=goal {
        if volume.is_air(pos) {
            if step == goal || !volume.is_air(pos - IVec3::Y) {
                let age = rng.next_i32_bound(3) as usize;
                volume.set(pos, cfg.weeping_vines_by_age[age]);
                break;
            }
            volume.set(pos, cfg.weeping_vines_plant);
        }
        pos.y -= 1;
    }
}

/// The reference breaks whatever stands on solid ground before a planted fungus
/// takes the cell; without item entities that is the clearing alone.
fn clear_below<W: WorldGenVolume>(volume: &mut W, pos: BlockPos) {
    if !volume.is_air(pos - IVec3::Y) {
        volume.set(pos, volume.world().air);
    }
}

fn replaceable<W: WorldGenVolume>(
    cfg: &CompiledHugeFungus,
    volume: &W,
    pos: BlockPos,
    check_plants: bool,
) -> bool {
    if volume.holds(&volume.world().replaceable, pos) {
        return true;
    }
    check_plants && cfg.replaceable_blocks.test(volume, pos)
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;
    use mcrs_voxel_storage::Blocks;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const NYLIUM: VoxelId = VoxelId(1);
    const STONE: VoxelId = VoxelId(2);
    const STEM: VoxelId = VoxelId(3);
    const HAT: VoxelId = VoxelId(4);
    const DECOR: VoxelId = VoxelId(5);
    const VINES_PLANT: VoxelId = VoxelId(6);
    const ORIGIN: BlockPos = BlockPos::new(8, 70, 8);

    fn config() -> CompiledHugeFungus {
        CompiledHugeFungus {
            valid_base: mask_of([NYLIUM.0]),
            stem_state: STEM,
            hat_state: HAT,
            hat_block: mask_of([HAT.0]),
            decor_state: DECOR,
            replaceable_blocks: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: StateMask::default(),
            },
            planted: false,
            place_vines: false,
            weeping_vines_plant: VINES_PLANT,
            weeping_vines_by_age: [VoxelId(7), VoxelId(8), VoxelId(9)],
        }
    }

    /// Stone everywhere but a one-block shaft over the nylium, so only the shaft
    /// is ever replaceable and the hat's cell-by-cell draws reduce to the one
    /// cell that sits above the stem.
    /// A volume whose only replaceable state is air.
    fn open_air() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.replaceable = mask_of([AIR.0]);
        volume
    }

    fn shaft() -> FakeVolume {
        let mut volume = open_air();
        for x in ORIGIN.x - 6..=ORIGIN.x + 6 {
            for z in ORIGIN.z - 6..=ORIGIN.z + 6 {
                for y in ORIGIN.y - 1..ORIGIN.y + 80 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        for y in ORIGIN.y..ORIGIN.y + 80 {
            volume.blocks.insert((ORIGIN.x, y, ORIGIN.z), AIR);
        }
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y - 1, ORIGIN.z), NYLIUM);
        volume
    }

    fn shape(seed: u64) -> (i32, bool, i32) {
        let mut rng = XoroshiroRandom::new(seed);
        let mut total_height = rng.next_i32_bound(10) + 4;
        if rng.next_i32_bound(12) == 0 {
            total_height *= 2;
        }
        let huge = rng.next_f32() < HUGE_PROBABILITY;
        let hat_height = (rng.next_i32_bound(1 + total_height / 3) + 5).min(total_height);
        (total_height, huge, hat_height)
    }

    /// The whole draw sequence of one fungus in a shaft: two for the height,
    /// one for the huge roll, one for the hat depth, one per hat layer, and the
    /// hat-block rolls of the single cell that crowns the stem.
    #[test]
    fn a_fungus_in_a_shaft_spends_exactly_the_layer_and_crown_draws() {
        let cfg = config();
        let seed = 0x5eed_f0e5;
        let (total_height, huge, hat_height) = shape(seed);
        assert!(!huge, "this seed must take the ordinary stem");

        let mut volume = shaft();
        let mut rng = XoroshiroRandom::new(seed);
        assert!(place_huge_fungus(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(seed);
        replay.next_i32_bound(10);
        replay.next_i32_bound(12);
        replay.next_f32();
        replay.next_i32_bound(1 + total_height / 3);
        let hat_start = total_height - hat_height;
        for dy in hat_start..=total_height {
            replay.next_i32_bound(3);
            if dy != total_height {
                continue;
            }
            // The crown cell is neither inside nor a corner, and never skirt.
            if replay.next_f32() >= 5.0e-4 && replay.next_f32() < 0.98 {
                replay.next_f32();
            }
        }
        assert_eq!(rng, replay, "one layer draw per layer, then the crown");

        for dy in 0..total_height {
            assert_eq!(volume.get(ORIGIN + IVec3::Y * dy), STEM);
        }
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, _, z), _)| *x == ORIGIN.x && *z == ORIGIN.z),
            "stone walls take nothing: {:?}",
            volume.writes
        );
    }

    /// Ground that is not the fungus's nylium refuses before the first draw.
    #[test]
    fn the_wrong_ground_draws_nothing() {
        let cfg = config();
        let mut volume = shaft();
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y - 1, ORIGIN.z), STONE);
        let mut rng = XoroshiroRandom::new(1);
        let before = rng.clone();
        assert!(!place_huge_fungus(&cfg, &mut volume, &mut rng, ORIGIN));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// The roof test sits between the height draws and the huge roll, so a
    /// fungus that would poke through the top costs two draws and no third.
    #[test]
    fn a_fungus_under_the_roof_stops_after_the_height_draws() {
        let cfg = config();
        let mut volume = open_air();
        let high = BlockPos::new(0, 380, 0);
        volume.blocks.insert((high.x, high.y - 1, high.z), NYLIUM);
        let mut rng = XoroshiroRandom::new(4);
        assert!(!place_huge_fungus(&cfg, &mut volume, &mut rng, high));

        let mut replay = XoroshiroRandom::new(4);
        replay.next_i32_bound(10);
        replay.next_i32_bound(12);
        assert_eq!(rng, replay, "the huge roll is never reached");
        assert!(volume.writes.is_empty());
    }

    /// Open air, hanging vines and all: nothing a fungus writes leaves the
    /// volume the writer allows it.
    #[test]
    fn the_hat_and_the_skirt_stay_inside_the_window() {
        let mut cfg = config();
        cfg.place_vines = true;
        for seed in 0..64u64 {
            let mut volume = open_air();
            volume
                .blocks
                .insert((ORIGIN.x, ORIGIN.y - 1, ORIGIN.z), NYLIUM);
            let mut rng = XoroshiroRandom::new(seed);
            place_huge_fungus(&cfg, &mut volume, &mut rng, ORIGIN);
            for ((x, _, z), _) in &volume.writes {
                assert!(
                    (x - ORIGIN.x).abs() <= 16 && (z - ORIGIN.z).abs() <= 16,
                    "seed {seed} wrote at {x},{z}"
                );
            }
        }
    }
}
