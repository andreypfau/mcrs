use bytemuck::{Pod, Zeroable};

/// The values of a slot that change with the ticks of the entity: the current values only. The
/// GPU keeps the record of the previous write of the slot.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TickRecord {
    pub block: [i32; 3],
    /// The position inside the block, in 1/65536 of a block.
    pub fraction: [u16; 3],
    /// In 1/65536 of a turn.
    pub body_yaw: u16,
    pub head_yaw: u16,
    /// The pitch in degrees divided by 180, as `snorm16`.
    pub pitch: i16,
    /// The bits of the walk position.
    pub walk_position: u32,
    /// The bits of the walk speed.
    pub walk_speed: u32,
}

/// The part of a slot that the entity cull reads for each slot.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct CullRecord {
    pub block: [i32; 3],
    /// The cull radius in blocks as an `f16`, rounded up.
    pub radius: u16,
    /// [`CULL_VALID`] and [`CULL_FULL_RIG_ONLY`].
    pub flags: u16,
}

pub const CULL_VALID: u16 = 1;
pub const CULL_FULL_RIG_ONLY: u16 = 2;

/// The state of a slot that changes less often than once a tick.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct StateRecord {
    pub definition: u16,
    /// Bit `l` is set when layer `l` draws.
    pub layer_mask: u8,
    pub spare: u8,
    /// The model and the variant of each layer.
    pub layers: [[u16; 2]; MAX_LAYERS],
    /// The `STATE_` flags in the low half, and the hidden palette entries 0 to 15 in the high half.
    pub flags: u32,
    /// The entity age at the last hurt.
    pub hurt_start: f32,
    /// The value of the entity clock at which the frame death time is 0.
    pub death_start: f32,
    pub block_light: u8,
    pub sky_light: u8,
    /// The scale of the entity as an `f16`.
    pub scale: u16,
    /// The slot of the section in the section table, or `u32::MAX`.
    pub section: u32,
    /// The client tick at which the age of the entity was 0, or the age while the clock stops.
    pub clock_offset: i32,
    /// The index of the input record, or `u32::MAX`.
    pub input: u32,
}

pub const MAX_LAYERS: usize = 4;
pub const STATE_BABY: u32 = 1;
pub const STATE_INVISIBLE: u32 = 2;
pub const STATE_UPSIDE_DOWN: u32 = 4;
pub const STATE_CLOCK_STOPPED: u32 = 8;
pub const STATE_DYING: u32 = 16;
pub const STATE_HIDDEN_SHIFT: u32 = 16;
/// The palette entries whose bones a render controller can hide.
pub const HIDEABLE_ENTRIES: usize = 16;

/// The values that the CPU computes for the pose of a slot in a frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct InputRecord {
    pub values: [f32; INPUT_VALUES],
    pub clips: [ClipSlot; CLIP_SLOTS],
}

pub const INPUT_VALUES: usize = 16;
pub const CLIP_SLOTS: usize = 4;

/// A clip instance that the CPU starts: the start is a value of the entity clock.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClipSlot {
    pub clip: u16,
    /// [`CLIP_PLAYING`].
    pub flags: u16,
    pub start: f32,
}

pub const CLIP_PLAYING: u16 = 1;

/// One tick record for the scatter pass: the slot, the client tick and the record.
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
const _: () = assert!(size_of::<InputRecord>() == 96);
const _: () = assert!(size_of::<ScatterEntry>() == 40);

/// The `f16` bits of `value`, rounded towards +∞. A value above the `f16` range gives the
/// largest finite `f16`.
#[must_use]
pub fn f16_up(value: f32) -> u16 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    const LARGEST: u16 = 0x7bff;
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    if exponent >= 31 {
        return LARGEST;
    }
    let mantissa = bits & 0x007f_ffff;
    if exponent <= 0 {
        let shift = (14 - exponent) as u32;
        if shift > 24 {
            return 1;
        }
        let full = mantissa | 0x0080_0000;
        let half = full >> shift;
        let exact = half << shift == full;
        return (half + u32::from(!exact)) as u16;
    }
    let half = ((exponent as u32) << 10) | (mantissa >> 13);
    let exact = mantissa & 0x1fff == 0;
    let rounded = half + u32::from(!exact);
    if rounded > u32::from(LARGEST) {
        LARGEST
    } else {
        rounded as u16
    }
}

/// The value of the `f16` bits `half`.
#[must_use]
pub fn f16_value(half: u16) -> f32 {
    let exponent = i32::from((half >> 10) & 0x1f);
    let mantissa = f32::from(half & 0x3ff);
    let sign = if half & 0x8000 != 0 { -1.0 } else { 1.0 };
    let magnitude = match exponent {
        0 => mantissa * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    };
    sign * magnitude
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_up_half_never_falls_below_the_value() {
        for value in [0.3f32, 1.0, 1.7, 2.513_7, 9.99, 0.000_1, 65_504.0, 1.0e-7] {
            let half = f16_up(value);
            assert!(f16_value(half) >= value, "{value} gave {}", f16_value(half));
        }
        assert_eq!(f16_value(f16_up(1.0)), 1.0);
        assert_eq!(f16_value(f16_up(1e9)), 65_504.0);
    }
}
