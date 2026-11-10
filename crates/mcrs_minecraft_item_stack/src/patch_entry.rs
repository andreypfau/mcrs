use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomValue;
use crate::UndecodedValue;
use crate::patch_key::PatchKey;

/// One entry of the component patch of a stack.
#[derive(Clone, Debug, PartialEq, Hash)]
pub enum PatchEntry {
    /// The stack has this component.
    Value(Builtin),
    /// The stack has no component of this type.
    Removal(BuiltinKey),
    /// The stack has this custom component.
    Custom(CustomValue),
    /// The stack has this component, which this crate does not decode.
    Undecoded(UndecodedValue),
    /// The stack has no component of the type with this identifier. The type has no Rust type in
    /// this crate.
    UndecodedRemoval(ResourceLocationBuf),
}

impl PatchEntry {
    pub(crate) fn key(&self) -> PatchKey<'_> {
        match self {
            Self::Value(value) => PatchKey::Builtin(value.key()),
            Self::Removal(key) => PatchKey::Builtin(*key),
            Self::Custom(value) => PatchKey::Custom(value.key),
            Self::Undecoded(value) => undecoded_key(value.name.as_location()),
            Self::UndecodedRemoval(name) => undecoded_key(name.as_location()),
        }
    }
}

// An undecoded value of a built-in type takes the place of that type in the order, so the patch
// never holds two entries for one type.
fn undecoded_key(name: ResourceLocation<'_>) -> PatchKey<'_> {
    match BuiltinKey::from_location(name) {
        Some(key) => PatchKey::Builtin(key),
        None => PatchKey::Undecoded(name),
    }
}
