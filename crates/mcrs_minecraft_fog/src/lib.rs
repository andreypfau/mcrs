//! The fog parameters of one frame of the client, computed from plain values of that frame.

#![no_std]

extern crate alloc;

mod drawn_rings;
mod fog_inputs;
mod fog_parameters;
mod fog_ramp;
mod follow_fog_end;

pub use drawn_rings::DrawnRings;
pub use fog_inputs::FogInputs;
pub use fog_parameters::FogParameters;
pub use fog_ramp::FogRamp;
pub use follow_fog_end::follow_fog_end;
