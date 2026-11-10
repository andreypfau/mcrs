use crate::BuiltinKey;

/// The position of a patch entry in the canonical order: the built-in types by type id, then the
/// custom types by key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchKey {
    Builtin(BuiltinKey),
    Custom(&'static str),
}
