use mcrs_minecraft_container_click::ButtonBinding;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GestureButton {
    pub number: u8,
    pub binding: ButtonBinding,
}

impl GestureButton {
    pub const LEFT: Self = Self {
        number: 0,
        binding: ButtonBinding::Other,
    };
    pub const RIGHT: Self = Self {
        number: 1,
        binding: ButtonBinding::Other,
    };
    pub const MIDDLE: Self = Self {
        number: 2,
        binding: ButtonBinding::PickItem,
    };
}
