/// How the amount of an attribute modifier changes the value of the attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

impl ModifierOperation {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::AddValue => "add_value",
            Self::AddMultipliedBase => "add_multiplied_base",
            Self::AddMultipliedTotal => "add_multiplied_total",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        [
            Self::AddValue,
            Self::AddMultipliedBase,
            Self::AddMultipliedTotal,
        ]
        .into_iter()
        .find(|operation| operation.name() == name)
    }
}
