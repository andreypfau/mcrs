use bytemuck::{Pod, Zeroable};
use half::f16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TickRecord {
    pub block: [i32; 3],
    pub fraction: [u16; 3],
    pub body_yaw: u16,
    pub head_yaw: u16,
    pub pitch: i16,
    pub walk_position: u32,
    pub walk_speed: u32,
}

impl TickRecord {
    #[must_use]
    pub fn turn_from_degrees(degrees: f32) -> u16 {
        ((degrees.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 % 65536) as u16
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CullRecord {
    pub block: [i32; 3],
    pub radius: u16,
    pub flags: u16,
}

pub const CULL_VALID: u16 = 1;
pub const CULL_FULL_RIG_ONLY: u16 = 2;
pub(crate) const CULL_VIEWS_SHIFT: u16 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct StateRecord {
    pub definition: u16,
    pub layer_mask: u8,
    pub spare: u8,
    pub layers: [[u16; 2]; MAX_LAYERS],
    pub flags: u32,
    pub hurt_start: f32,
    pub death_start: f32,
    pub block_light: u8,
    pub sky_light: u8,
    pub scale: u16,
    pub section: u32,
    pub clock_offset: i32,
    pub input: u32,
}

pub const MAX_LAYERS: usize = 4;
pub const STATE_BABY: u32 = 1;
pub const STATE_DYING: u32 = 16;
pub const STATE_HIDDEN_SHIFT: u32 = 16;
pub const HIDEABLE_ENTRIES: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct InputRecord {
    pub values: [f32; INPUT_VALUES],
    pub clips: [ClipSlot; CLIP_SLOTS],
}

pub const INPUT_VALUES: usize = 32;
pub const CLIP_SLOTS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClipSlot {
    pub clip: u16,
    pub flags: u16,
    pub start: f32,
}

pub const CLIP_PLAYING: u16 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ScatterEntry {
    pub slot: u32,
    pub tick: u32,
    pub record: TickRecord,
}

const _: () = assert!(size_of::<TickRecord>() == 32);
const _: () = assert!(size_of::<CullRecord>() == 16);
const _: () = assert!(size_of::<StateRecord>() == 48);
const _: () = assert!(size_of::<InputRecord>() == 160);
const _: () = assert!(size_of::<ScatterEntry>() == 40);

#[must_use]
pub fn f16_up(value: f32) -> u16 {
    debug_assert!(
        value.is_nan() || value >= 0.0,
        "a size is never negative, and {value} is"
    );
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    let nearest = f16::from_f32(value);
    let below = nearest.to_f32() < value;
    nearest
        .to_bits()
        .saturating_add(u16::from(below))
        .min(f16::MAX.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_up_half_never_falls_below_the_value() {
        let value_of = |half: u16| f16::from_bits(half).to_f32();
        for value in [0.3f32, 1.0, 1.7, 2.513_7, 9.99, 0.000_1, 65_504.0, 1.0e-7] {
            let half = f16_up(value);
            assert!(value_of(half) >= value, "{value} gave {}", value_of(half));
        }
        assert_eq!(value_of(f16_up(1.0)), 1.0);
        assert_eq!(value_of(f16_up(65_510.0)), 65_504.0);
        assert_eq!(value_of(f16_up(1e9)), 65_504.0);
    }
}
