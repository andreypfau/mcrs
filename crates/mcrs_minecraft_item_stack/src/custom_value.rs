use alloc::boxed::Box;

/// The encoded bytes of one [`CustomComponent`](crate::CustomComponent) in a patch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomValue {
    pub key: &'static str,
    pub bytes: Box<[u8]>,
}
