#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveEffect {
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub visible: bool,
}
