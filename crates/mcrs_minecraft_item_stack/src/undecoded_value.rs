use alloc::boxed::Box;

use mcrs_minecraft_resource::ResourceLocationBuf;

/// The saved form of one component that this crate does not decode.
///
/// `bytes` holds one NBT value without a name: the tag id, then the payload, big-endian. The
/// writer of the saved form writes the value back unchanged. No other format can carry it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UndecodedValue {
    /// The identifier of the component type.
    pub name: ResourceLocationBuf,
    pub bytes: Box<[u8]>,
}
