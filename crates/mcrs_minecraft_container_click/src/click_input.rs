use crate::Gesture;
use crate::StackPresence;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClickInput {
    pub gesture: Gesture,
    pub shift: bool,
    pub control: bool,
    pub creative: bool,
    pub cursor: StackPresence,
    pub target: StackPresence,
}
