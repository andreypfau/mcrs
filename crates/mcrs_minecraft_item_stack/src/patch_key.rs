use mcrs_minecraft_resource::ResourceLocation;

use crate::BuiltinKey;
use crate::ComponentType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchKey<'name> {
    Builtin(BuiltinKey),
    Undecoded(ResourceLocation<'name>),
    UndecodedWire(ComponentType),
    Custom(&'static str),
}
