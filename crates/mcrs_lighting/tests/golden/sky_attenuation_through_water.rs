// A flowing water block placed in an otherwise empty column; the sky-light
// engine should attenuate sky values by one level per layer of water below
// the surface (water's light dampening is 1).

use mcrs_protocol::BlockStateId;

const WATER_FLOWING: BlockStateId = BlockStateId(0x1002);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((8, 10, 8), WATER_FLOWING)];

pub const EXPECTED_SKY_LIGHT: [u8; 2048] =
    super::expected::compute_sky_field(&[((8, 10, 8), 1)]);
