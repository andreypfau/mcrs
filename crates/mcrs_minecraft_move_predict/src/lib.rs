//! Layer 2 of the movement checks: each client tick of a player runs through the physics model
//! on the side of its client, for each state that the client can have, and the result is
//! compared with what the client reports.

mod candidates;
mod change;
mod client_tick;
mod client_view;
mod kept_changes;
mod limits;
mod outcome;
mod prediction;
mod push_bound;
mod pusher_history;

pub use change::Change;
pub use client_tick::ChangeStates;
pub use client_tick::ClientTick;
pub use client_view::ClientSection;
pub use client_view::ClientView;
pub use limits::Limits;
pub use outcome::Mode;
pub use outcome::Outcome;
pub use prediction::Prediction;
pub use pusher_history::Pusher;
pub use pusher_history::PusherHistory;
