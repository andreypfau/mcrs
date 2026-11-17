use mcrs_minecraft_wire::{Decode, Encode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub enum LookAnchor {
    #[default]
    Feet,
    Eyes,
}
