use mcrs_minecraft_container_click::{Clicked, StackPresence};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureHit {
    pub clicked: Option<Clicked>,
    pub entry: Option<u16>,
    pub target: Option<StackPresence>,
    pub takes_left_press: bool,
    pub permits_drag: bool,
}
