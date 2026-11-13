//! The survival dig kernel: the rate of one tick, the progress sum of a dig, its crack stage and
//! what a finish action does. It holds numbers only: no position, no packet and no game mode.
#![no_std]

mod dig;
mod finish;
mod miner;
mod rate;

pub use dig::Dig;
pub use finish::Finish;
pub use miner::Miner;
pub use rate::rate;
