//! What the server benchmarks share: the server child process, the status probe, the tick-time
//! parsers and maths, and the upkeep a headless protocol client owes the server.

mod client;
mod clock;
mod kind;
mod process;
mod status;
mod ticks;

pub use client::{TICK, Upkeep, VIEW_DISTANCE, announce_loaded, answer_upkeep, connect};
pub use clock::unix_now;
pub use kind::{Kind, TickMetric};
pub use process::{Line, ServerProcess, Stop};
pub use status::answers as answers_status;
pub use ticks::{
    TickWindow, WINDOW_TICKS, mcrs_tick_millis, mcrs_windows, mean, nearest_rank, round,
    tick_windows,
};
