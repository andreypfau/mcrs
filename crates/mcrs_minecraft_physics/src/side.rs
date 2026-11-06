/// The side that runs a step: the flags, gravity and the bounce differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The client of the player that it simulates.
    OwnClient,
    /// The server; `authority` is true when the server simulates the motion of the entity.
    Server { authority: bool },
}

impl Side {
    /// True when this side simulates the motion of the entity.
    #[must_use]
    pub const fn has_authority(self) -> bool {
        match self {
            Side::OwnClient => true,
            Side::Server { authority } => authority,
        }
    }
}
