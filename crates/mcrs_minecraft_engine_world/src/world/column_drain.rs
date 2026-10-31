use bevy_ecs::schedule::{ScheduleLabel, SystemSet};

/// Puts the columns whose sections landed on the wire, at the end of every tick and between
/// ticks, so a column read from the save does not wait for the next tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnDrain;

/// The stages of a [`ColumnDrain`], in run order. Each acts only on what is pending.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnDrainSet {
    /// Finished columns come off the pool into their sections.
    Collect,
    /// The column index, block entities and heightmaps catch up with what landed.
    Reconcile,
    /// Ticket levels spread, sections spawn, and the columns they need are requested.
    Request,
    /// A finished light epoch is retired and the next one dispatched.
    Light,
    /// Ready columns go out and the outbox is flushed towards the host.
    Send,
}
