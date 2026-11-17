use mcrs_minecraft_physics::Teleport;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub mode: Mode,
    pub offset: Option<f64>,
    pub wrong_on_ground: bool,
    pub wrong_horizontal_collision: bool,
    pub setback: Option<Teleport>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Predicted,
    Coarse,
    Resync,
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
