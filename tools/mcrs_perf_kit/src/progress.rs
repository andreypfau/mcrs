use std::time::{Duration, Instant};

use crate::section::Section;

/// Prints one line a step of a section, with the time the rest of the section should take.
pub struct Progress {
    section: Section,
    steps: usize,
    done: usize,
    started: Instant,
    estimate: Duration,
}

impl Progress {
    /// `estimate` is the expected time of one step, used until a step has finished.
    #[must_use]
    pub fn new(section: Section, steps: usize, estimate: Duration) -> Self {
        Self {
            section,
            steps,
            done: 0,
            started: Instant::now(),
            estimate,
        }
    }

    pub fn step(&mut self, label: &str) {
        let remaining = u32::try_from(self.steps.saturating_sub(self.done)).unwrap_or(u32::MAX);
        let per_step = match u32::try_from(self.done) {
            Ok(done) if done > 0 => self.started.elapsed() / done,
            _ => self.estimate,
        };
        let eta = per_step.saturating_mul(remaining).as_secs();
        println!(
            "[{}] {label} ... step {}/{}, section ETA {}m {:02}s",
            self.section,
            self.done.saturating_add(1),
            self.steps,
            eta / 60,
            eta % 60
        );
        self.done = self.done.saturating_add(1);
    }
}
