use mcrs_voxel::BlockPos;

/// `BlockPos.asLong`: 26 bits of x, 26 of z and 12 of y, packed x-z-y.
pub const fn block_pos_as_long(pos: BlockPos) -> i64 {
    const HORIZONTAL_MASK: i64 = (1 << 26) - 1;
    const Y_MASK: i64 = (1 << 12) - 1;
    let pos = pos.as_ivec3();
    ((pos.x as i64 & HORIZONTAL_MASK) << 38)
        | ((pos.z as i64 & HORIZONTAL_MASK) << 12)
        | (pos.y as i64 & Y_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_long_packs_like_the_reference() {
        assert_eq!(block_pos_as_long(BlockPos::new(0, 0, 0)), 0);
        assert_eq!(
            block_pos_as_long(BlockPos::new(1, 2, 3)),
            (1 << 38) | (3 << 12) | 2
        );
        assert_eq!(block_pos_as_long(BlockPos::new(-1, -1, -1)), -1);
        assert_eq!(
            block_pos_as_long(BlockPos::new(-16, 62, 25)),
            -4398046408642
        );
    }
}
