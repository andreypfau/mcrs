/// The side that runs a step: the flags, gravity and the bounce differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Side {
    /// The client of the player that it simulates.
    OwnClient,
    /// The server; `authority` is true when the server simulates the motion of the entity.
    Server { authority: bool },
    /// A client that steps an entity whose authority is the server. It moves the entity as the
    /// server does, but it does not push an item out of a solid block and does not apply the
    /// ground rule of a falling block.
    Viewer,
}

impl Side {
    /// True when this side simulates the motion of the entity.
    #[must_use]
    pub const fn has_authority(self) -> bool {
        match self {
            Side::OwnClient | Side::Viewer => true,
            Side::Server { authority } => authority,
        }
    }

    /// True when this side pushes an item out of a solid block and lands a falling block on the
    /// ground. A viewer leaves both to the server.
    #[must_use]
    pub const fn applies_corrections(self) -> bool {
        matches!(self, Side::OwnClient | Side::Server { authority: true })
    }
}
