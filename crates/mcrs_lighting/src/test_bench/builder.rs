use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft::world::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

pub fn from_input(input: &[((i32, i32, i32), BlockStateId)]) -> BlockPalette {
    let mut palette = BlockPalette::default();
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
        let expected = BlockPalette::default();
        assert_eq!(
            palette.get(BlockPos::new(0, 0, 0)),
            expected.get(BlockPos::new(0, 0, 0))
        );
    }

    #[test]
    fn single_entry_round_trips_through_get() {
        let palette = from_input(&[((1, 2, 3), BlockStateId(0x1000))]);
        assert_eq!(palette.get(BlockPos::new(1, 2, 3)), BlockStateId(0x1000));
    }

    #[test]
    fn duplicate_coordinates_last_write_wins() {
        let palette = from_input(&[
            ((5, 5, 5), BlockStateId(0x1000)),
            ((5, 5, 5), BlockStateId(0x1001)),
        ]);
        assert_eq!(palette.get(BlockPos::new(5, 5, 5)), BlockStateId(0x1001));
    }
}
