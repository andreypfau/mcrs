use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomValue;
use crate::UndecodedValue;
use crate::UndecodedWireValue;
use crate::patch_key::PatchKey;

#[derive(Clone, Debug, PartialEq, Hash)]
pub enum PatchEntry {
    Value(Builtin),
    Removal(BuiltinKey),
    Custom(CustomValue),
    Undecoded(UndecodedValue),
    UndecodedRemoval(ResourceLocationBuf),
    UndecodedWire(UndecodedWireValue),
}

impl PatchEntry {
    pub(crate) fn key(&self) -> PatchKey<'_> {
        match self {
            Self::Value(value) => PatchKey::Builtin(value.key()),
            Self::Removal(key) => PatchKey::Builtin(*key),
            Self::Custom(value) => PatchKey::Custom(value.key),
            Self::Undecoded(value) => undecoded_key(value.name.as_location()),
            Self::UndecodedRemoval(name) => undecoded_key(name.as_location()),
            Self::UndecodedWire(value) => value.key(),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn builtin_key(&self) -> Option<BuiltinKey> {
        match self.key() {
            PatchKey::Builtin(key) => Some(key),
            PatchKey::Undecoded(_) | PatchKey::UndecodedWire(_) | PatchKey::Custom(_) => None,
        }
    }
}

fn undecoded_key(name: ResourceLocation<'_>) -> PatchKey<'_> {
    match BuiltinKey::from_location(name) {
        Some(key) => PatchKey::Builtin(key),
        None => PatchKey::Undecoded(name),
    }
}
