// A flowing water block placed in an otherwise empty chunk: the sky-light
// engine drops the level by `max(1, dampening) == 1` at the water cell. Light
// in surrounding cells reaches the column below the water laterally (the
// engine's 3D BFS wraps around the obstacle): every cell directly under the
// water in column (x=8, z=8) reads 14 because the same-y neighbour columns
// are at 15 and one lateral step attenuates by 1. Above the water and in
// every other column the engine reports the all-air maximum of 15.

use mcrs_protocol::BlockStateId;

const WATER_FLOWING: BlockStateId = BlockStateId(0x1002);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((8, 10, 8), WATER_FLOWING)];

pub const EXPECTED_SKY_LIGHT: [u8; 2048] = build_expected();

const fn build_expected() -> [u8; 2048] {
    let mut out = [0xFFu8; 2048];
    // Column (x=8, z=8): cells at y in 0..=10 store the value 14 instead of 15.
    let mut y = 0;
    while y <= 10 {
        let idx = (y << 8) | (8 << 4) | 8;
        let byte_index = idx >> 1;
        let shift = ((idx & 1) * 4) as u32;
        let mask = !(0x0Fu8 << shift);
        out[byte_index] = (out[byte_index] & mask) | (14u8 << shift);
        y += 1;
    }
    out
}
