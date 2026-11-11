use alloc::vec::Vec;

use mcrs_minecraft_nbt::split_wire;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;

use crate::BuiltinKey;
use crate::WireError;

// The measure of the quota is not specified; this reader counts the bytes of the network NBT. The
// NBT reader enforces the depth limit of 512 levels.
const NBT_QUOTA: usize = 2_097_152;

pub(crate) fn read_text(input: &mut &[u8], component: BuiltinKey) -> Result<Text, WireError> {
    let mut tag = split_wire(input).map_err(|error| WireError::of_value(&error, component))?;
    if tag.len() > NBT_QUOTA {
        return Err(WireError::BadValue(component));
    }
    let text = Text::decode(&mut tag).map_err(|error| WireError::of_value(&error, component))?;
    if !tag.is_empty() {
        return Err(WireError::BadValue(component));
    }
    Ok(text)
}

pub(crate) fn write_text(
    text: &Text,
    component: BuiltinKey,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    text.encode(&mut *output)
        .map_err(|_| WireError::BadValue(component))
}
