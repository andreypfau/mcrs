use crate::ClickKind;
use crate::Clicked;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub kind: ClickKind,
    pub button: u8,
    pub clicked: Clicked,
}
