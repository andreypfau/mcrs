use mcrs_minecraft_brain::Part;
use mcrs_minecraft_brain::Turn;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gate {
    due: Option<u64>,
}

impl Gate {
    pub fn passes<W: ?Sized>(&mut self, turn: &Turn<'_, W>, part: Part, chance: f64) -> bool {
        let due = self.due.unwrap_or_else(|| {
            let wait = turn.random(part).trials_to_success(chance);
            turn.cadence.nth_evaluation(turn.tick, wait)
        });
        if due <= turn.tick {
            self.due = None;
            true
        } else {
            self.due = Some(due);
            false
        }
    }

    pub fn end(&mut self) {
        self.due = None;
    }

    #[must_use]
    pub const fn due(&self) -> Option<u64> {
        self.due
    }
}
