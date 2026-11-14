use mcrs_minecraft_entity::InstanceModifier;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// A change of one attribute instance (ATR-INS-002 and ATR-INS-004).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InstanceEdit {
    /// Stores the base value as given.
    SetBase(f64),
    /// The call "set": leaves an equal modifier in place and puts any other at the end of the
    /// sequence in place of the modifier of its identifier.
    Put(InstanceModifier),
    /// Removes the modifier of this identifier.
    Remove(ResourceLocationBuf),
}
