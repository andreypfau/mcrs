#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
    pub auto_jump: bool,
}
