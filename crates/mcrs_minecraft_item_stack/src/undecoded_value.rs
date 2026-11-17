use alloc::boxed::Box;

use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UndecodedValue {
    pub name: ResourceLocationBuf,
    pub bytes: Box<[u8]>,
}
