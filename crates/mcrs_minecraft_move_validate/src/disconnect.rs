use core::fmt;

/// Why a check ends the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disconnect {
    /// A move or an accept has a coordinate or an angle that is NaN or infinite, or a second
    /// position packet arrived in one client tick.
    InvalidPlayerMovement,
    /// A vehicle move has a coordinate or an angle that is NaN or infinite.
    InvalidVehicleMovement,
    /// The packet does not have the size or the values that its fields allow.
    Malformed,
}

impl Disconnect {
    /// The translation key of the reason, or `None` when the game shows a generic reason.
    #[must_use]
    pub fn translation_key(self) -> Option<&'static str> {
        match self {
            Self::InvalidPlayerMovement => Some("multiplayer.disconnect.invalid_player_movement"),
            Self::InvalidVehicleMovement => Some("multiplayer.disconnect.invalid_vehicle_movement"),
            Self::Malformed => None,
        }
    }
}

impl fmt::Display for Disconnect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPlayerMovement => "invalid player movement",
            Self::InvalidVehicleMovement => "invalid vehicle movement",
            Self::Malformed => "a movement packet of a wrong size or value",
        })
    }
}

impl core::error::Error for Disconnect {}
