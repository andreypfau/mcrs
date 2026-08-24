/// The seven booleans `Input.java` packs into one byte: `forward` 1, `backward`
/// 2, `left` 4, `right` 8, `jump` 16, `shift` 32, `sprint` 64. That ordering is
/// not ours to choose later.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Input {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

impl Input {
    pub const EMPTY: Self = Self {
        forward: false,
        backward: false,
        left: false,
        right: false,
        jump: false,
        shift: false,
        sprint: false,
    };
}
