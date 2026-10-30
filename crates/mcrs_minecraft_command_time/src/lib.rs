//! The tick count, time marker and timeline arguments.

// std: mcrs_minecraft_text, which holds the text of every argument, needs it.
mod error;
mod time;
mod time_lookup;

pub use error::TimeError;
pub use time::{TimeArgument, TimeMarkerArgument, TimelineArgument};
pub use time_lookup::TimeLookup;
