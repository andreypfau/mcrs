#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SwingKind {
    None,
    #[default]
    Whack,
    Stab,
}

impl SwingKind {
    pub(crate) const ALL: [Self; 3] = [Self::None, Self::Whack, Self::Stab];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Whack => "whack",
            Self::Stab => "stab",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}
