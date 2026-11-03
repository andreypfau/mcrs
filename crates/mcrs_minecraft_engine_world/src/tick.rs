use bevy_ecs::schedule::SystemSet;

/// The phases of a dimension's `FixedUpdate`, run in declaration order by `DimensionPlugin`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickSet {
    /// What the world generation and light services finished since the last tick lands in the
    /// world. It is the only point at which their results change what the simulation sees, so
    /// the state a tick starts from does not depend on when between ticks they arrived.
    ServiceResults,
    /// What clients sent this tick becomes authoritative state.
    Input,
    /// Entities advance and request the world writes they cause.
    Simulate,
    /// The block writes requested this tick land in the sections.
    WorldWrite,
    /// Derived state catches up with the tick's writes.
    Project,
    /// What the tick changed leaves the dimension.
    Output,
}
