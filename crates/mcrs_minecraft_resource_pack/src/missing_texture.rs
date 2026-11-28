pub const MISSING_SIZE: u32 = 16;
const MAGENTA: [u8; 4] = [0xF8, 0x00, 0xF8, 0xFF];
const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

#[must_use]
pub fn missing_pixels() -> Vec<u8> {
    let half = MISSING_SIZE / 2;
    (0..MISSING_SIZE)
        .flat_map(|y| (0..MISSING_SIZE).map(move |x| (x < half) != (y < half)))
        .flat_map(|magenta| if magenta { MAGENTA } else { BLACK })
        .collect()
}
