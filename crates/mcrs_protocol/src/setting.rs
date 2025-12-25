use bevy_ecs::prelude::Component;
use bitfield_struct::bitfield;
use mcrs_protocol_macros::{Decode, Encode};

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Component, Encode, Decode)]
pub enum ChatMode {
    Enabled,
    CommandsOnly,
    #[default]
    Hidden,
}

#[bitfield(u8)]
#[derive(PartialEq, Eq, Encode, Decode)]
pub struct DisplayedSkinParts {
    pub cape: bool,
    pub jacket: bool,
    pub left_sleeve: bool,
    pub right_sleeve: bool,
    pub left_pants_leg: bool,
    pub right_pants_leg: bool,
    pub hat: bool,
    _pad: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Encode, Decode)]
pub enum MainArm {
    Left,
    #[default]
    Right,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub enum ParticleStatus {
    All,
    Decreased,
    Minimal,
}
