use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::Turn;

use crate::Gate;
use crate::LOOK_AROUND_START;
use crate::LOOK_AROUND_WAIT;
use crate::LOOK_CHANCE;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LookAroundGoal {
    gate: Gate,
    ends: Option<u64>,
}

impl LookAroundGoal {
    pub fn can_start<W: ?Sized>(&mut self, turn: &Turn<'_, W>) -> bool {
        self.gate.passes(turn, LOOK_AROUND_WAIT, LOOK_CHANCE)
    }

    pub fn start<W: ?Sized>(&mut self, turn: &Turn<'_, W>, intents: &mut Intents) {
        let mut random = turn.random(LOOK_AROUND_START);
        let angle = random.unit() * core::f64::consts::TAU;
        let look_time = 20 + u64::from(random.below(20));
        self.ends = Some(
            turn.cadence
                .next_evaluation(turn.tick.saturating_add(look_time + 1)),
        );
        intents.look = Some(LookIntent::Around {
            cos: libm::cos(angle) as f32,
            sin: libm::sin(angle) as f32,
        });
    }

    #[must_use]
    pub fn can_continue<W: ?Sized>(&self, turn: &Turn<'_, W>) -> bool {
        self.ends.is_some_and(|ends| turn.tick < ends)
    }

    pub fn stop(&mut self, intents: &mut Intents) {
        self.ends = None;
        intents.look = Some(LookIntent::EndAround);
    }

    #[must_use]
    pub const fn ends(&self) -> Option<u64> {
        self.ends
    }

    #[must_use]
    pub const fn gate(&self) -> Gate {
        self.gate
    }

    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }
}
