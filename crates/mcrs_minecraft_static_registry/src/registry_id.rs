use std::format;
use std::io::Write;

use mcrs_minecraft_wire::{Context, Decode, Encode, Result, VarInt};

pub fn encode_registry_id(id: u16, w: impl Write) -> Result<()> {
    VarInt(i32::from(id)).encode(w)
}

pub fn decode_registry_id(r: &mut &[u8]) -> Result<u16> {
    let VarInt(raw) = VarInt::decode(r)?;
    u16::try_from(raw)
        .ok()
        .with_context(|| format!("registry id {raw} is outside 0..=65535"))
}
