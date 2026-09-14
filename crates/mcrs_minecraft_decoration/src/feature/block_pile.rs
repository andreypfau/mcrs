use crate::feature::holds;
use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};

use crate::feature::tree::provider::StateProvider;

/// One `block_pile` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledBlockPile {
    pub state_provider: StateProvider,
    /// A path draws a coin instead of answering from its shape.
    pub dirt_path: StateMask,
}

/// The floor the reference refuses to pile on, measured from the dimension's
/// bottom.
const MIN_HEIGHT_ABOVE_BOTTOM: i32 = 5;

/// `BlockPileFeature.place`: a box two cells tall around the origin, visited in
/// the reference's x, then y, then z order, with a radial test whose threshold
/// is two fresh floats at every cell.
pub fn place_block_pile<W: WorldGenVolume>(
    cfg: &CompiledBlockPile,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    if at.y < volume.extent().min_y + MIN_HEIGHT_ABOVE_BOTTOM {
        return false;
    }

    let x_radius = 2 + rng.next_i32_bound(2);
    let z_radius = 2 + rng.next_i32_bound(2);

    for z in at.z - z_radius..=at.z + z_radius {
        for y in at.y..=at.y + 1 {
            for x in at.x - x_radius..=at.x + x_radius {
                let dx = at.x - x;
                let dz = at.z - z;
                let inside =
                    (dx * dx + dz * dz) as f32 <= rng.next_f32() * 10.0 - rng.next_f32() * 6.0;
                if inside || rng.next_f32() < 0.031 {
                    try_place(cfg, volume, rng, BlockPos::new(x, y, z));
                }
            }
        }
    }
    true
}

fn try_place<W: WorldGenVolume>(
    cfg: &CompiledBlockPile,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    pos: BlockPos,
) {
    if !volume.is_air(pos) {
        return;
    }
    let below = volume.get(pos - IVec3::Y);
    // `isFaceSturdy(UP)` as the full-collision-cube flag: a top slab reads as
    // no ground where the reference piles on it.
    let may_place = if holds(&cfg.dirt_path, below) {
        rng.next_bool()
    } else {
        holds(&volume.world().sturdy_up, below)
    };
    if !may_place {
        return;
    }
    let state = cfg.state_provider.state(volume, rng, pos);
    volume.set(pos, state);
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;
    use mcrs_minecraft_chunk::VoxelId;

    const SNOW: VoxelId = VoxelId(2);
    const DIRT: VoxelId = VoxelId(3);
    const PATH: VoxelId = VoxelId(4);
    const AT: BlockPos = BlockPos::new(0, 64, 0);

    fn config() -> CompiledBlockPile {
        CompiledBlockPile {
            state_provider: StateProvider::Simple(SNOW),
            dirt_path: mask_of([PATH]),
        }
    }

    fn ground(state: VoxelId) -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.sturdy_up = mask_of([DIRT, PATH]);
        for x in -8..=8 {
            for z in -8..=8 {
                volume.blocks.insert((x, AT.y - 1, z), state);
            }
        }
        volume
    }

    /// The two radii, then two floats at every cell of the box and a third
    /// wherever the radial test failed, in x, then y, then z order.
    #[test]
    fn every_cell_of_the_box_spends_its_floats_in_the_references_order() {
        let cfg = config();
        let mut volume = ground(DIRT);
        let mut rng = XoroshiroRandom::new(4242);

        assert!(place_block_pile(&cfg, &mut volume, &mut rng, AT));

        let mut replay = XoroshiroRandom::new(4242);
        let x_radius = 2 + replay.next_i32_bound(2);
        let z_radius = 2 + replay.next_i32_bound(2);
        let mut expected = Vec::new();
        for z in AT.z - z_radius..=AT.z + z_radius {
            for y in AT.y..=AT.y + 1 {
                for x in AT.x - x_radius..=AT.x + x_radius {
                    let dx = AT.x - x;
                    let dz = AT.z - z;
                    let inside = (dx * dx + dz * dz) as f32
                        <= replay.next_f32() * 10.0 - replay.next_f32() * 6.0;
                    if inside || replay.next_f32() < 0.031 {
                        // Only the lower layer stands on dirt; the upper one
                        // stands on whatever the lower layer left, which is
                        // air until this cell is written.
                        if y == AT.y {
                            expected.push((x, y, z));
                        }
                    }
                }
            }
        }
        assert_eq!(rng, replay, "two radii and two or three floats per cell");

        let written: Vec<(i32, i32, i32)> = volume.writes.iter().map(|(pos, _)| *pos).collect();
        assert_eq!(written, expected);
        assert!(volume.writes.iter().all(|(_, state)| *state == SNOW));
    }

    /// A path below draws a coin in place of the shape test, which shifts every
    /// later draw of the pile.
    #[test]
    fn a_path_below_spends_a_draw_the_shape_test_would_not() {
        let cfg = config();
        let mut plain = ground(DIRT);
        let mut paths = ground(PATH);
        let mut plain_rng = XoroshiroRandom::new(11);
        let mut path_rng = XoroshiroRandom::new(11);

        place_block_pile(&cfg, &mut plain, &mut plain_rng, AT);
        place_block_pile(&cfg, &mut paths, &mut path_rng, AT);

        assert_ne!(plain_rng, path_rng, "the coin per candidate cell");
    }

    /// The reference refuses to pile within five blocks of the bottom of the
    /// world, before it draws anything.
    #[test]
    fn a_pile_too_close_to_the_bottom_of_the_world_draws_nothing() {
        let cfg = config();
        let mut volume = ground(DIRT);
        let mut rng = XoroshiroRandom::new(1);
        let before = rng.clone();

        assert!(!place_block_pile(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, -60, 0)
        ));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }
}
