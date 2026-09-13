use bevy_math::IVec3;
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug)]
pub struct CompiledFillLayer {
    /// Counted up from the volume's own floor, not from zero.
    pub height: i32,
    pub state: VoxelId,
}

/// `FillLayerFeature.place`: one whole chunk square at a single height, air
/// only, and it always reports success.
///
/// The square is the origin's chunk, so the column's x and z are floored to
/// sixteen rather than taken as written.
pub fn place_fill_layer<W: WorldGenVolume>(
    cfg: &CompiledFillLayer,
    volume: &mut W,
    at: IVec3,
) -> bool {
    let y = volume.extent().min_y + cfg.height;
    for dx in 0..16 {
        for dz in 0..16 {
            let pos = IVec3::new(at.x + dx, y, at.z + dz);
            if volume.is_air(pos) {
                volume.set(pos, cfg.state);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen::feature::placer::{BoxRegion, WorldStates, mask_of};
    use mcrs_voxel_storage::{Blocks, BlocksMut};

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);
    const SNOW: VoxelId = VoxelId(2);

    fn volume() -> BoxRegion {
        let mut volume = BoxRegion::new(IVec3::new(0, -64, 0), IVec3::new(31, -50, 31), AIR);
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            ..WorldStates::default()
        };
        volume
    }

    #[test]
    fn it_fills_one_chunk_square_at_one_height_and_only_over_air() {
        let mut volume = volume();
        volume.set(IVec3::new(3, -60, 4), STONE);

        let cfg = CompiledFillLayer {
            height: 4,
            state: SNOW,
        };
        assert!(place_fill_layer(&cfg, &mut volume, IVec3::new(0, 0, 0)));

        assert_eq!(volume.get(IVec3::new(0, -60, 0)), SNOW);
        assert_eq!(volume.get(IVec3::new(15, -60, 15)), SNOW);
        assert_eq!(
            volume.get(IVec3::new(3, -60, 4)),
            STONE,
            "a filled cell is left alone"
        );
        assert_eq!(
            volume.get(IVec3::new(16, -60, 0)),
            AIR,
            "the square stops at sixteen"
        );
        assert_eq!(
            volume.get(IVec3::new(0, -59, 0)),
            AIR,
            "one height, not a column"
        );
    }

    #[test]
    fn the_height_counts_from_the_window_floor() {
        let mut volume = volume();
        let cfg = CompiledFillLayer {
            height: 0,
            state: SNOW,
        };
        place_fill_layer(&cfg, &mut volume, IVec3::ZERO);
        assert_eq!(volume.get(IVec3::new(0, -64, 0)), SNOW);
    }
}
