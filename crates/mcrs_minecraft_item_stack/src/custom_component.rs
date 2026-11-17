use alloc::vec::Vec;
use core::hash::Hash;

pub trait CustomComponent: Clone + PartialEq + Hash {
    const KEY: &'static str;

    fn encode(&self, bytes: &mut Vec<u8>);

    fn decode(bytes: &[u8]) -> Option<Self>;
}
