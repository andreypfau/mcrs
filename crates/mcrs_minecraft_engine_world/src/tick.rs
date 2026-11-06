use bevy_ecs::schedule::SystemSet;

/// The phases of a dimension's `FixedUpdate`, run in declaration order by `DimensionPlugin`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickSet {
    /// The writes of the domain steps that ended beside the tick, such as world generation and
    /// lighting, land in the world. It is the only point at which they change what the
    /// simulation sees, so the state a tick starts from does not depend on when the steps ended.
    DomainInput,
    /// What clients sent this tick becomes authoritative state.
    ClientInput,
    /// Entities advance and request the world writes they cause.
    Simulate,
    /// The block writes requested this tick land in the sections.
    WorldWrite,
    /// Derived state catches up with the tick's writes.
    Project,
    /// What the tick changed leaves the dimension.
    Output,
}
