/// Whether the stack that goes in can join an alike stack. A stack with a damage value above 0
/// stays alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacking {
    Stacks,
    Alone,
}
