/// How the amount of an attribute modifier changes the value of the attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}
