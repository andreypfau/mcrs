use std::collections::VecDeque;
use std::iter::Cycle;
use std::time::Duration;
use std::time::Instant;
use std::vec;

pub(crate) struct Delay<T> {
    samples: Cycle<vec::IntoIter<Duration>>,
    queue: VecDeque<(Instant, T)>,
}

impl<T> Delay<T> {
    pub(crate) fn new(samples: Vec<Duration>) -> Self {
        Self {
            samples: samples.into_iter().cycle(),
            queue: VecDeque::new(),
        }
    }

    pub(crate) fn push(&mut self, now: Instant, item: T) {
        let delay = self
            .samples
            .next()
            .map_or(Duration::ZERO, |round_trip| round_trip / 2);
        let previous = self.queue.back().map_or(now, |(due, _)| *due);
        self.queue.push_back((previous.max(now + delay), item));
    }

    pub(crate) fn pop_due(&mut self, now: Instant) -> Option<T> {
        self.queue
            .pop_front_if(|(due, _)| *due <= now)
            .map(|(_, item)| item)
    }
}

pub(crate) fn parse_trace(text: &str) -> anyhow::Result<Vec<Duration>> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(|line| Ok(Duration::try_from_secs_f64(line.parse::<f64>()? / 1000.0)?))
        .collect()
}
