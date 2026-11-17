use mcrs_minecraft_nbt::split_wire;

use crate::BuiltinKey;
use crate::WireError;

const NBT_QUOTA: usize = 2_097_152;

pub(crate) fn split_nbt<'a>(
    input: &mut &'a [u8],
    component: BuiltinKey,
) -> Result<&'a [u8], WireError> {
    let tag = split_wire(input).map_err(|error| WireError::of_value(&error, component))?;
    if tag.len() > NBT_QUOTA {
        return Err(WireError::BadValue(component));
    }
    Ok(tag)
}
