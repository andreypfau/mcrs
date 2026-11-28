use crate::BrainRules;
use crate::Cadence;
use crate::Intents;
use crate::KeyedRandom;
use crate::MobView;
use crate::Part;
use crate::PlayerRow;
use crate::WorldSeed;

pub trait MobBrain<W: ?Sized> {
    fn rules(&self) -> &'static BrainRules;

    fn evaluate(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) -> u64;
}

#[derive(Debug)]
pub struct Turn<'a, W: ?Sized> {
    pub tick: u64,
    pub seed: WorldSeed,
    pub max_lag: u8,
    pub cadence: Cadence,
    pub view: &'a MobView,
    pub players: &'a [PlayerRow],
    pub world: &'a W,
}

impl<W: ?Sized> Turn<'_, W> {
    #[must_use]
    pub fn random(&self, part: Part) -> KeyedRandom {
        KeyedRandom::new(self.seed, &self.view.key, self.tick, part)
    }

    #[must_use]
    pub fn next_evaluation(&self) -> u64 {
        self.cadence.next_evaluation(self.tick.saturating_add(1))
    }
}
