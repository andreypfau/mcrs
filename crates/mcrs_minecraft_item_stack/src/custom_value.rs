use alloc::boxed::Box;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CustomValue {
    pub key: &'static str,
    pub bytes: Box<[u8]>,
}
