use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;

/// What a world publishes about a mob in a tile delta. A spawn carries [`MobReport::Spawn`], a
/// move [`MobReport::Move`], and a data event [`MobReport::Signal`].
#[derive(Clone, Debug, PartialEq)]
pub enum MobReport {
    Spawn(MobSpawn),
    /// The state of a mob at the end of a tick in which it changed.
    Move(MobState),
    Signal(MobSignal),
}
