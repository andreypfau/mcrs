use std::io::Write;

use bevy_math::DVec3;
use derive_more::{From, Into};

use crate::{Decode, Encode, VarInt};

/// A vector quantized into three 15-bit components sharing one integer scale.
///
/// Components larger in magnitude than [`LpVec3::ABS_MAX_VALUE`] are clamped,
/// NaN becomes zero, and a vector whose largest component is below
/// [`LpVec3::ABS_MIN_VALUE`] encodes as a single zero byte.
#[derive(Copy, Clone, PartialEq, Default, Debug, From, Into)]
pub struct LpVec3(pub DVec3);

impl LpVec3 {
    pub const ABS_MAX_VALUE: f64 = 1.7179869183E10;
    pub const ABS_MIN_VALUE: f64 = 3.051944088384301E-5;
}

const MAX_QUANTIZED_VALUE: f64 = 32766.0;
const DATA_BITS_MASK: u64 = 32767;
const CONTINUATION_FLAG: u64 = 4;
const X_OFFSET: u32 = 3;
const Y_OFFSET: u32 = 18;
const Z_OFFSET: u32 = 33;

fn sanitize(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-LpVec3::ABS_MAX_VALUE, LpVec3::ABS_MAX_VALUE)
    }
}

fn pack(value: f64) -> u64 {
    ((value * 0.5 + 0.5) * MAX_QUANTIZED_VALUE).round() as u64
}

fn unpack(value: u64) -> f64 {
    ((value & DATA_BITS_MASK) as f64).min(MAX_QUANTIZED_VALUE) * 2.0 / MAX_QUANTIZED_VALUE - 1.0
}

impl Encode for LpVec3 {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let [x, y, z] = [self.0.x, self.0.y, self.0.z].map(sanitize);
        let chessboard_length = x.abs().max(y.abs()).max(z.abs());
        if chessboard_length < Self::ABS_MIN_VALUE {
            return 0u8.encode(w);
        }

        let scale = chessboard_length.ceil() as u64;
        let is_partial = scale & 3 != scale;
        let markers = if is_partial {
            scale & 3 | CONTINUATION_FLAG
        } else {
            scale
        };
        let divisor = scale as f64;
        let buffer = markers
            | pack(x / divisor) << X_OFFSET
            | pack(y / divisor) << Y_OFFSET
            | pack(z / divisor) << Z_OFFSET;

        (buffer as u8).encode(&mut w)?;
        ((buffer >> 8) as u8).encode(&mut w)?;
        ((buffer >> 16) as u32).encode(&mut w)?;
        if is_partial {
            VarInt((scale >> 2) as i32).encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for LpVec3 {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let lowest = u8::decode(r)? as u64;
        if lowest == 0 {
            return Ok(Self(DVec3::ZERO));
        }

        let middle = u8::decode(r)? as u64;
        let highest = u32::decode(r)? as u64;
        let buffer = highest << 16 | middle << 8 | lowest;

        let mut scale = lowest & 3;
        if lowest & CONTINUATION_FLAG == CONTINUATION_FLAG {
            scale |= (VarInt::decode(r)?.0 as u32 as u64) << 2;
        }
        let scale = scale as f64;

        Ok(Self(DVec3::new(
            unpack(buffer >> X_OFFSET) * scale,
            unpack(buffer >> Y_OFFSET) * scale,
            unpack(buffer >> Z_OFFSET) * scale,
        )))
    }
}
