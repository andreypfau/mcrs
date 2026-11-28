#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Side {
    OwnClient,
    Server { authority: bool },
    Viewer,
}

impl Side {
    #[must_use]
    pub const fn has_authority(self) -> bool {
        match self {
            Side::OwnClient | Side::Viewer => true,
            Side::Server { authority } => authority,
        }
    }

    #[must_use]
    pub const fn applies_corrections(self) -> bool {
        matches!(self, Side::OwnClient | Side::Server { authority: true })
    }
}
