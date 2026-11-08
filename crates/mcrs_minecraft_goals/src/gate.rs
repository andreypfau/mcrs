use mcrs_minecraft_brain::Part;
use mcrs_minecraft_brain::Turn;

/// The random chance in the start condition of a goal, as a wait (GOAL-WAKE-003): the first full
/// evaluation in which the goal is eligible and has no wait draws the count of eligible
/// evaluations up to the pass, from a geometric law with the chance of the gate. The gate passes
/// in the last of them. The goal must be eligible in each full evaluation between; else the wait
/// ends. Thus the mob can sleep until the due evaluation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gate {
    due: Option<u64>,
}

impl Gate {
    /// The goal is eligible in this full evaluation: true when the gate passes now. A pass ends
    /// the wait.
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

    /// The goal is not eligible in this full evaluation: its wait ends.
    pub fn end(&mut self) {
        self.due = None;
    }

    /// The full evaluation in which the wait is due.
    #[must_use]
    pub const fn due(&self) -> Option<u64> {
        self.due
    }
}
