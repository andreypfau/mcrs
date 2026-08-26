use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;
use mcrs_voxel_world::voxel_update::SectionVoxels;

pub fn from_input(input: &[((i32, i32, i32), VoxelId)]) -> SectionVoxels {
    let mut palette = SectionVoxels::default();
    for &((x, y, z), id) in input {
        palette.set(BlockPos::new(x, y, z), id);
    }
    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_matches_default_palette() {
        let palette = from_input(&[]);
        let expected = SectionVoxels::default();
        assert_eq!(
            palette.get(BlockPos::new(0, 0, 0)),
            expected.get(BlockPos::new(0, 0, 0))
        );
    }

    #[test]
    fn single_entry_round_trips_through_get() {
        let palette = from_input(&[((1, 2, 3), VoxelId(0x1000))]);
        assert_eq!(palette.get(BlockPos::new(1, 2, 3)), VoxelId(0x1000));
    }

    #[test]
    fn duplicate_coordinates_last_write_wins() {
        let palette = from_input(&[((5, 5, 5), VoxelId(0x1000)), ((5, 5, 5), VoxelId(0x1001))]);
        assert_eq!(palette.get(BlockPos::new(5, 5, 5)), VoxelId(0x1001));
    }
}
