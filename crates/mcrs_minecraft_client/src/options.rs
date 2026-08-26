//! Vanilla `Options` at their 26.3 defaults, standing in for a settings screen.
//!
//! Three more defaults are load-bearing by their absence rather than their
//! value, so they are named here and nowhere else: `fovEffectScale` 1.0
//! collapses `Mth.lerp(scale, 1.0, modifier)` to `modifier`, and `invertMouseX`,
//! `invertMouseY` and `smoothCamera` off are what leave the inversions and the
//! `SmoothDouble` branch of `MouseHandler.turnPlayer` unreachable.

pub const FOV: f32 = 70.0;
pub const SENSITIVITY: f32 = 0.5;
pub const SPRINT_WINDOW_TICKS: u8 = 7;
