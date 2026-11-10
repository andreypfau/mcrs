use bevy_ecs::resource::Resource;

/// The number of the last ticks whose tick time the host keeps.
pub const TICK_SAMPLES: usize = 100;

/// The pace of the server tick: the tick times of the last [`TICK_SAMPLES`] ticks and the
/// sprint that runs ticks with no wait. The server loop of the host owns it; each dimension
/// world reads a copy.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TickPace {
    target_nanos: u64,
    samples: [u64; TICK_SAMPLES],
    ticks: u64,
    sprint: Option<Sprint>,
    sprint_tick: bool,
    report: Option<SprintReport>,
}

/// A run of ticks with no wait between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sprint {
    remaining: u64,
    ran: u64,
    nanos: u64,
}

/// The ticks that a sprint ran and their wall time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SprintReport {
    pub ticks: u64,
    pub nanos: u64,
}

impl SprintReport {
    /// The ticks in one second, rounded toward 0.
    #[must_use]
    pub fn ticks_per_second(self) -> i32 {
        let millis = self.millis();
        i32::try_from((1000.0 * self.ticks as f64 / millis) as i64).unwrap_or(i32::MAX)
    }

    /// The milliseconds per tick, or `target_millis` when the sprint ran no tick.
    #[must_use]
    pub fn millis_per_tick(self, target_millis: f64) -> f64 {
        if self.ticks == 0 {
            target_millis
        } else {
            self.millis() / self.ticks as f64
        }
    }

    fn millis(self) -> f64 {
        self.nanos.max(1) as f64 / 1.0e6
    }
}

impl TickPace {
    #[must_use]
    pub fn new(tick_rate: u32) -> Self {
        Self {
            target_nanos: 1_000_000_000 / u64::from(tick_rate.max(1)),
            samples: [0; TICK_SAMPLES],
            ticks: 0,
            sprint: None,
            sprint_tick: false,
            report: None,
        }
    }

    /// The target tick time, in nanoseconds.
    #[must_use]
    pub fn target_nanos(&self) -> u64 {
        self.target_nanos
    }

    /// The tick time of each of the last [`TICK_SAMPLES`] ticks, in nanoseconds; a slot that no
    /// tick wrote yet holds 0.
    #[must_use]
    pub fn samples(&self) -> &[u64; TICK_SAMPLES] {
        &self.samples
    }

    /// The sum of the samples divided by the number of ticks that wrote one, at least 1.
    #[must_use]
    pub fn average_nanos(&self) -> u64 {
        let written = self.ticks.clamp(1, TICK_SAMPLES as u64);
        let sum = self
            .samples
            .iter()
            .fold(0u64, |sum, &sample| sum.saturating_add(sample));
        sum / written
    }

    #[must_use]
    pub fn is_sprinting(&self) -> bool {
        self.sprint.is_some()
    }

    /// Starts a sprint of `ticks` ticks. A sprint that runs ends with no report, and the
    /// return says whether one ran.
    pub fn start_sprint(&mut self, ticks: u64) -> bool {
        self.sprint
            .replace(Sprint {
                remaining: ticks,
                ran: 0,
                nanos: 0,
            })
            .is_some()
    }

    /// Ends the sprint that runs with its report. Returns `false` when no sprint runs.
    pub fn stop_sprint(&mut self) -> bool {
        let Some(sprint) = self.sprint.take() else {
            return false;
        };
        self.report = Some(SprintReport {
            ticks: sprint.ran,
            nanos: sprint.nanos,
        });
        true
    }

    /// The report of a sprint that ended since the last call.
    pub fn take_report(&mut self) -> Option<SprintReport> {
        self.report.take()
    }

    /// At the start of a tick: a sprint with no remaining tick ends here, and a sprint with one
    /// counts this tick.
    pub fn begin_tick(&mut self) {
        self.sprint_tick = false;
        match &mut self.sprint {
            Some(sprint) if sprint.remaining == 0 => {
                self.stop_sprint();
            }
            Some(sprint) => {
                sprint.remaining = sprint.remaining.saturating_sub(1);
                self.sprint_tick = true;
            }
            None => {}
        }
    }

    /// At the end of the work of a tick that took `nanos`. Returns whether the next tick starts
    /// at once, with no wait.
    pub fn end_tick(&mut self, nanos: u64) -> bool {
        let slot = (self.ticks % TICK_SAMPLES as u64) as usize;
        if let Some(sample) = self.samples.get_mut(slot) {
            *sample = nanos;
        }
        self.ticks = self.ticks.saturating_add(1);
        if let Some(sprint) = self.sprint.as_mut().filter(|_| self.sprint_tick) {
            sprint.ran = sprint.ran.saturating_add(1);
            sprint.nanos = sprint.nanos.saturating_add(nanos);
        }
        self.sprint.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sprint_of_n_ticks_runs_n_ticks_and_ends_at_the_start_of_the_next() {
        let mut pace = TickPace::new(20);
        assert!(!pace.start_sprint(3), "no sprint ran before");
        let mut waits = Vec::new();
        for _ in 0..5 {
            pace.begin_tick();
            waits.push(!pace.end_tick(10_000_000));
        }
        assert_eq!(waits, [false, false, false, true, true]);
        assert_eq!(
            pace.take_report(),
            Some(SprintReport {
                ticks: 3,
                nanos: 30_000_000
            })
        );
        assert_eq!(pace.take_report(), None);
    }

    #[test]
    fn the_report_divides_by_the_ticks_and_the_average_by_the_written_samples() {
        let report = SprintReport {
            ticks: 200,
            nanos: 1_000_000_000,
        };
        assert_eq!(report.ticks_per_second(), 200);
        assert_eq!(report.millis_per_tick(50.0), 5.0);
        let idle = SprintReport { ticks: 0, nanos: 0 };
        assert_eq!(idle.millis_per_tick(50.0), 50.0);

        let mut pace = TickPace::new(20);
        pace.end_tick(4);
        pace.end_tick(8);
        assert_eq!(pace.average_nanos(), 6, "two ticks wrote a sample");
    }
}
