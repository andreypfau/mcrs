/// The base values of the attributes of a new entity of a type that differ from the attribute
/// defaults (`entity-tracking.md`, TRK-TYPE-003 and TRK-TYPE-004).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BaseAttributes {
    /// The base of `max_health`, which is also the health of a new entity.
    pub max_health: f32,
    /// The base of `movement_speed`, widened from its f32 value where the type gives one.
    pub movement_speed: f64,
}
