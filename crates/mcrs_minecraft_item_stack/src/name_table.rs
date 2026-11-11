/// A set of names that a codec writes as identifiers and a value type holds as numeric ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NameTable {
    Item,
    Enchantment,
    Attribute,
    Block,
    BlockTag,
    ComponentType,
}
