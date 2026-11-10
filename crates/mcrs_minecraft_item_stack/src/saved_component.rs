use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;

use crate::Builtin;
use crate::BuiltinKey;
use crate::NameLookup;
use crate::SavedError;
use crate::saved::read_value;

/// Reads one component from the identifier of its type and its saved form, as the reader of the
/// saved stack reads the value of one patch key.
///
/// `Ok(None)` means that the type has no Rust type in this crate. The function does not check
/// that such a type exists.
pub fn read_saved_component<L: NameLookup>(
    component: ResourceLocation<'_>,
    value: &NbtTag,
    names: &L,
) -> Result<Option<Builtin>, SavedError> {
    match BuiltinKey::from_location(component) {
        Some(key) => read_value(key, value, names).map(Some),
        None => Ok(None),
    }
}
