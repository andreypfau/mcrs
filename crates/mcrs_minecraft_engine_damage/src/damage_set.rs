use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DamageSet {
    PacketCollect,
    PacketFilter,
    PacketApply,
    PacketReport,
    Collect,
    Filter,
    Apply,
    Report,
}
