use crate::ButtonBinding;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Gesture {
    Press { button: u8, binding: ButtonBinding },
    Release { button: u8, binding: ButtonBinding },
    HotbarKey(u8),
    OffhandKey,
    DropKey,
    PickKey,
}
