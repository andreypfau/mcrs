use alloc::vec::Vec;

use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;

use crate::BuiltinKey;
use crate::WireError;
use crate::split_nbt::split_nbt;

pub(crate) fn read_text(input: &mut &[u8], component: BuiltinKey) -> Result<Text, WireError> {
    let mut tag = split_nbt(input, component)?;
    let text = mcrs_minecraft_nbt::without_nbt_budget(|| Text::decode(&mut tag))
        .map_err(|error| WireError::of_value(&error, component))?;
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
