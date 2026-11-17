use alloc::string::String;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockStateProperty {
    pub name: String,
    pub value: String,
}
