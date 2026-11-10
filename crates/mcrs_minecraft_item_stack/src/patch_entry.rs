use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomValue;
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
}

impl PatchEntry {
    pub(crate) fn key(&self) -> PatchKey {
        match self {
            Self::Value(value) => PatchKey::Builtin(value.key()),
            Self::Removal(key) => PatchKey::Builtin(*key),
            Self::Custom(value) => PatchKey::Custom(value.key),
        }
    }
}
