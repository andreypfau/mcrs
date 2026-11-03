use mcrs_minecraft_light_engine::epoch::LightUpdate;

use crate::to_light::ToLight;

/// Everything a light actor waits on: its dimension and its epochs.
pub(crate) enum LightInbox {
    Dimension(ToLight),
    Finished(FinishedEpoch),
}

/// The output of the epoch with `id`. `None` when the epoch ended without one: it panicked,
/// or it was cancelled.
pub(crate) struct FinishedEpoch {
    pub(crate) id: u64,
    pub(crate) update: Option<LightUpdate>,
}
