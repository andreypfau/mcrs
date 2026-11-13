//! The options a user sets on the client, as one plain value.
#![no_std]

mod client_options;
mod options_error;

pub use client_options::{
    ClientOptions, FOV_RANGE, MAX_FRAMERATE_RANGE, MAX_FRAMERATE_STEP, VIEW_DISTANCE_DEFAULT,
    VIEW_DISTANCE_RANGE,
};
pub use options_error::OptionsError;
