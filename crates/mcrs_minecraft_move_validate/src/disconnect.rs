use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disconnect {
    InvalidPlayerMovement,
    InvalidVehicleMovement,
    Malformed,
}

impl Disconnect {
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
