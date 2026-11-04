//! The link between two parts of the engine. Owned messages go in, owned messages come out,
//! and the placement of the far end decides where its [`Stage`] runs.

// std: flume

mod in_flight;
mod inline_end;
mod link;
mod placement;
mod reply;
mod stage;
mod step;
mod task_end;
mod tasks;
mod thread_end;

pub use in_flight::InFlight;
pub use inline_end::InlineEnd;
pub use link::Link;
pub use placement::{ParsePlacementError, Placement};
pub use stage::Stage;
pub use step::Step;
pub use task_end::{TaskEnd, TaskId, TaskOutcome};
