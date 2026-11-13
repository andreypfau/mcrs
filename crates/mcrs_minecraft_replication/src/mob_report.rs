use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;

/// What a world publishes about a mob or an item entity in a tile delta. A spawn carries
/// [`MobReport::Spawn`], a move [`MobReport::Move`], and a data event [`MobReport::Signal`] or
/// [`MobReport::Stack`].
#[derive(Clone, Debug, PartialEq)]
pub enum MobReport {
    Spawn(MobSpawn),
    /// The state of a mob at the end of a tick in which it changed.
    Move(MobState),
    Signal(MobSignal),
    /// The new stack of an item entity (IEN-NET-003).
    Stack(ProtoStack<Wire>),
}
