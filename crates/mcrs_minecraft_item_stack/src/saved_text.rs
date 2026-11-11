use alloc::borrow::Cow;
use alloc::vec::Vec;

use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_text::Text;

use crate::BuiltinKey;
use crate::Lore;
use crate::SavedError;
use crate::lore::MAX_LINES;

pub(crate) fn read_text(tag: &NbtTag, component: BuiltinKey) -> Result<Text, SavedError> {
    from_tag(tag.clone()).map_err(|_| SavedError::BadValue(component))
}

pub(crate) fn read_lore(tag: &NbtTag) -> Result<Lore, SavedError> {
    let NbtTag::List(list) = tag else {
        return Err(SavedError::BadValue(BuiltinKey::Lore));
    };
    if list.len() > MAX_LINES {
        return Err(SavedError::OutOfRange(BuiltinKey::Lore));
    }
    let mut lines: Vec<Text> = Vec::with_capacity(list.len());
    for line in list {
        lines.push(read_text(line, BuiltinKey::Lore)?);
    }
    Ok(Lore {
        lines: Cow::Owned(lines),
    })
}
