//! The 50,000-pig benchmark harness: one server, one client mode and the phases of a run.
//! See `README.md` in `benchmarks/mob-bench` for the method.

pub mod adapter;
pub mod args;
mod os;
#[cfg(target_os = "macos")]
mod os_macos;
#[cfg(windows)]
mod os_windows;
mod phase;
pub mod probe;
pub mod report;
pub mod run;
pub mod scene;
pub mod session;
mod stir;
pub mod watcher;
