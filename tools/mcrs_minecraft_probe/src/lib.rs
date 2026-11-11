//! Scripted clients that run one scenario against a server over TCP and record what they observe.

mod client;
mod known_difference;
mod player_ban;
mod run;
mod scenario;
mod trace;

pub use known_difference::KnownDifference;
pub use player_ban::PlayerBan;
pub use run::run;
pub use scenario::{Action, Scenario, Step, Target};
pub use trace::{ConnectionState, Disconnect, Observed, StepTrace, Trace, TraceVersion};
