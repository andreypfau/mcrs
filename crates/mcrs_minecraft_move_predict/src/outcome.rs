use mcrs_minecraft_physics::Teleport;

/// What layer 2 made of one client tick or one accept.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub mode: Mode,
    /// The offset of a violation of the offset check, in blocks.
    pub offset: Option<f64>,
    /// No kept candidate has the reported on-ground bit.
    pub wrong_on_ground: bool,
    /// No kept candidate has the reported horizontal collision bit.
    pub wrong_horizontal_collision: bool,
    /// A correction to send as `player_position` with the next teleport id. The prediction
    /// counts it as pending until [`crate::Prediction::accept`].
    pub setback: Option<Teleport>,
}

/// The class of a client tick: which checks it gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Each candidate ran the physics model.
    #[default]
    Predicted,
    /// Too many candidates or pushers: the reported position is accepted with an unknown
    /// velocity.
    Coarse,
    /// The first position after a coarse tick: the reported position is accepted, with the
    /// velocity that the change of position gives.
    Resync,
    /// A teleport is pending: the prediction runs with no check.
    Exempt,
}

impl From<Mode> for Outcome {
    fn from(mode: Mode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }
}
