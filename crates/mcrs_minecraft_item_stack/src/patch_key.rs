use mcrs_minecraft_resource::ResourceLocation;

use crate::BuiltinKey;

/// The position of a patch entry in the canonical order: the built-in types by type id, then the
/// types that this crate does not decode by identifier, then the custom types by key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchKey<'name> {
    Builtin(BuiltinKey),
    Undecoded(ResourceLocation<'name>),
    Custom(&'static str),
}
