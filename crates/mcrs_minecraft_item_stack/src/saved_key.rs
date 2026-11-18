use alloc::string::String;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;

use crate::SavedError;

pub(crate) const REMOVAL_MARKER: &str = "!";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyKind {
    Value,
    Removal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SavedKey<'key> {
    pub(crate) kind: KeyKind,
    pub(crate) name: ResourceLocation<'key>,
}

impl<'key> SavedKey<'key> {
    pub(crate) fn read(key: &'key str) -> Result<Self, SavedError> {
        let (kind, name) = match key.strip_prefix(REMOVAL_MARKER) {
            Some(name) => (KeyKind::Removal, name),
            None => (KeyKind::Value, key),
        };
        let name = ResourceLocation::read(name).map_err(|_| SavedError::BadKey)?;
        Ok(Self { kind, name })
    }

    pub(crate) fn is_repeated(self, earlier: &[(String, NbtTag)]) -> bool {
        earlier
            .iter()
            .any(|(key, _)| SavedKey::read(key).is_ok_and(|key| key == self))
    }
}
