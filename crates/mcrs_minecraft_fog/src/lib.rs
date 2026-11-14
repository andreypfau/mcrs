//! The fog parameters of one frame of the client, computed from plain values of that frame.

#![no_std]

mod fog_inputs;
mod fog_parameters;
mod fog_ramp;

pub use fog_inputs::FogInputs;
pub use fog_parameters::FogParameters;
pub use fog_ramp::FogRamp;
