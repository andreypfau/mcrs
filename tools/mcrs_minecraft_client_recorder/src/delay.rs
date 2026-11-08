use std::collections::VecDeque;
use std::iter::Cycle;
use std::time::Duration;
use std::time::Instant;
use std::vec;

/// Holds the data of one direction between the client and the proxy for the one-way delays of
/// a trace. Each item waits for half of the next round trip sample, and never passes an item
/// that came before it. An item is one frame from the client, or the bytes of one poll for the
/// client.
pub(crate) struct Delay<T> {
    samples: Cycle<vec::IntoIter<Duration>>,
    queue: VecDeque<(Instant, T)>,
}

impl<T> Delay<T> {
    /// No sample: each item passes at once.
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

/// Reads a trace: one round trip time in milliseconds on each line; `#` starts a comment.
pub(crate) fn parse_trace(text: &str) -> anyhow::Result<Vec<Duration>> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(|line| Ok(Duration::try_from_secs_f64(line.parse::<f64>()? / 1000.0)?))
        .collect()
}
