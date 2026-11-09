use mcrs_minecraft_brain::GoalWorld;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::Turn;

use crate::PigBrain;

/// The brain of each mob type with goals: one variant for each type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VanillaBrain {
    Pig(PigBrain),
}

impl<W: GoalWorld + ?Sized> MobBrain<W> for VanillaBrain {
    const PLAYER_RANGE: f64 = <PigBrain as MobBrain<W>>::PLAYER_RANGE;
    const IDLE_LIMIT: u32 = <PigBrain as MobBrain<W>>::IDLE_LIMIT;

    fn evaluate(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) -> u64 {
        match self {
            Self::Pig(brain) => brain.evaluate(turn, intents),
        }
    }
}
