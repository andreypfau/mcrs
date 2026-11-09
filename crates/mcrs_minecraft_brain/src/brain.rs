use crate::Cadence;
use crate::Intents;
use crate::KeyedRandom;
use crate::MobView;
use crate::Part;
use crate::PlayerRow;
use crate::WorldSeed;

/// The brain of one mob type: its brain state and one full evaluation of its goals. `W` is the
/// read-only world of the host for the queries of the goals.
pub trait MobBrain<W: ?Sized> {
    /// The largest horizontal distance from the mob at which an evaluation reads a player.
    const PLAYER_RANGE: f64;

    /// The idle time from which a reset of the idle time wakes the brain (GOAL-WAKE-005, event
    /// 5): the idle time at which the goals stop to start a stroll; `u32::MAX` for none.
    const IDLE_LIMIT: u32;

    /// One full evaluation from the snapshot of `turn.tick`. Writes the guidance changes to
    /// `intents`, and returns the next tick at which the brain needs a full evaluation when no fact
    /// wakes it; `u64::MAX` for none.
    fn evaluate(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) -> u64;
}

/// The inputs of one full evaluation of one mob.
#[derive(Debug)]
pub struct Turn<'a, W: ?Sized> {
    pub tick: u64,
    pub seed: WorldSeed,
    pub max_lag: u8,
    pub cadence: Cadence,
    pub view: &'a MobView,
    /// The players near the tile of the mob, in the order of their order keys: each player within
    /// [`MobBrain::PLAYER_RANGE`] of the mob is in it.
    pub players: &'a [PlayerRow],
    pub world: &'a W,
}

impl<W: ?Sized> Turn<'_, W> {
    /// The keyed stream of `part` for this mob and this issue tick.
    #[must_use]
    pub fn random(&self, part: Part) -> KeyedRandom {
        KeyedRandom::new(self.seed, &self.view.key, self.tick, part)
    }

    /// The first full evaluation after this one.
    #[must_use]
    pub fn next_evaluation(&self) -> u64 {
        self.cadence.next_evaluation(self.tick.saturating_add(1))
    }
}
