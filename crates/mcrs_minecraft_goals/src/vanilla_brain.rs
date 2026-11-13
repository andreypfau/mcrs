use mcrs_minecraft_brain::BrainRules;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::Turn;
use mcrs_minecraft_goal_world::GoalWorld;

use crate::ChickenBrain;
use crate::CowBrain;
use crate::PigBrain;
use crate::SheepBrain;

/// The brain of each mob type with goals: one variant for each type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VanillaBrain {
    Pig(PigBrain),
    Cow(CowBrain),
    Sheep(SheepBrain),
    Chicken(ChickenBrain),
}

impl<W: GoalWorld + ?Sized> MobBrain<W> for VanillaBrain {
    fn rules(&self) -> &'static BrainRules {
        match self {
            Self::Pig(brain) => MobBrain::<W>::rules(brain),
            Self::Cow(brain) => MobBrain::<W>::rules(brain),
            Self::Sheep(brain) => MobBrain::<W>::rules(brain),
            Self::Chicken(brain) => MobBrain::<W>::rules(brain),
        }
    }

    fn evaluate(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) -> u64 {
        match self {
            Self::Pig(brain) => brain.evaluate(turn, intents),
            Self::Cow(brain) => brain.evaluate(turn, intents),
            Self::Sheep(brain) => brain.evaluate(turn, intents),
            Self::Chicken(brain) => brain.evaluate(turn, intents),
        }
    }
}
