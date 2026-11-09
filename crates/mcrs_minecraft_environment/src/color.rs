use std::fmt::Write as _;

use mcrs_minecraft_nbt::tag::NbtTag;

/// The channel a byte of a packed colour gives.
#[must_use]
pub const fn channel(byte: u32) -> f32 {
    (byte & 0xFF) as f32 / 255.0
}

/// The byte a channel writes as.
#[must_use]
pub fn byte(channel: f32) -> u8 {
    // The cast rounds toward zero, which is the floor of a clamped channel; a NaN casts to 0.
    (channel.clamp(0.0, 1.0) * 255.0) as u8
}

#[must_use]
pub const fn rgb(packed: u32) -> [f32; 3] {
    [channel(packed >> 16), channel(packed >> 8), channel(packed)]
}

#[must_use]
pub const fn argb(packed: u32) -> [f32; 4] {
    [
        channel(packed >> 16),
        channel(packed >> 8),
        channel(packed),
        channel(packed >> 24),
    ]
}

/// `#rrggbb`.
#[must_use]
pub fn rgb_hex([red, green, blue]: [f32; 3]) -> String {
    hex(&[red, green, blue])
}

/// `#aarrggbb`.
#[must_use]
pub fn argb_hex([red, green, blue, alpha]: [f32; 4]) -> String {
    hex(&[alpha, red, green, blue])
}

fn hex(channels: &[f32]) -> String {
    let mut text = String::with_capacity(1 + 2 * channels.len());
    text.push('#');
    for channel in channels {
        let _ = write!(text, "{:02x}", byte(*channel));
    }
    text
}

fn integer(tag: &NbtTag) -> Option<i64> {
    match tag {
        NbtTag::Byte(value) => Some(i64::from(*value)),
        NbtTag::Short(value) => Some(i64::from(*value)),
        NbtTag::Int(value) => Some(i64::from(*value)),
        NbtTag::Long(value) => Some(*value),
        _ => None,
    }
}

pub(crate) fn float(tag: &NbtTag) -> Option<f32> {
    match tag {
        NbtTag::Float(value) => Some(*value),
        NbtTag::Double(value) => Some(*value as f32),
        other => integer(other).map(|value| value as f32),
    }
}

fn packed(text: &str, digits: usize) -> Option<u32> {
    let hex = text.strip_prefix('#').filter(|hex| hex.len() == digits)?;
    u32::from_str_radix(hex, 16).ok()
}

fn channels<const N: usize>(tag: &NbtTag) -> Option<[f32; N]> {
    let NbtTag::List(items) = tag else {
        return None;
    };
    let mut out = [0.0; N];
    if items.len() != N {
        return None;
    }
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = float(item)?;
    }
    Some(out)
}

pub(crate) fn read_rgb(tag: &NbtTag) -> Result<[f32; 3], String> {
    let read = match tag {
        NbtTag::String(text) => packed(text, 6).map(rgb),
        NbtTag::List(_) => channels::<3>(tag),
        other => integer(other).map(|value| rgb(value as u32)),
    };
    read.ok_or_else(|| "expected `#rrggbb`, an int or three floats".to_owned())
}

pub(crate) fn read_argb(tag: &NbtTag) -> Result<[f32; 4], String> {
    let read = match tag {
        NbtTag::String(text) => packed(text, 8).map(argb),
        NbtTag::List(_) => channels::<4>(tag),
        other => integer(other).map(|value| argb(value as u32)),
    };
    read.ok_or_else(|| "expected `#aarrggbb`, an int or four floats".to_owned())
}
