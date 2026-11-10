use alloc::vec::Vec;
use core::hash::Hash;

/// A component type that a user adds to the built-in types.
///
/// `KEY` identifies the type in a patch. Two types with the same key are one type for a stack.
pub trait CustomComponent: Clone + PartialEq + Hash {
    const KEY: &'static str;

    /// Appends the encoded value to `bytes`.
    fn encode(&self, bytes: &mut Vec<u8>);

    /// Gives `None` for bytes that [`encode`](Self::encode) does not write.
    fn decode(bytes: &[u8]) -> Option<Self>;
}
