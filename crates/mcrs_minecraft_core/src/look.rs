use mcrs_minecraft_wire::{Decode, Encode};

#[derive(Copy, Clone, PartialEq, Default, Debug, Encode, Decode)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}
