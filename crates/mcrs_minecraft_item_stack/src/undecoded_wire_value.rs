use alloc::boxed::Box;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::patch_key::PatchKey;

/// The wire form of one component that this crate does not decode.
///
/// Only the reader of the wire stack with length prefix makes it, because only that format
/// states where the component ends. Each writer of a wire stack writes the bytes back unchanged.
/// The saved form cannot carry it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UndecodedWireValue {
    component: ComponentType,
    // The built-in type with this type id, which the reader found in the table of component types.
    builtin: Option<BuiltinKey>,
    bytes: Box<[u8]>,
}

impl UndecodedWireValue {
    pub(crate) fn new(component: ComponentType, builtin: Option<BuiltinKey>, bytes: &[u8]) -> Self {
        Self {
            component,
            builtin,
            bytes: bytes.into(),
        }
    }

    /// The type id of the component type.
    #[must_use]
    pub fn component(&self) -> ComponentType {
        self.component
    }

    /// The wire form of the component, without its length prefix.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn key(&self) -> PatchKey<'static> {
        match self.builtin {
            Some(key) => PatchKey::Builtin(key),
            None => PatchKey::UndecodedWire(self.component),
        }
    }
}
