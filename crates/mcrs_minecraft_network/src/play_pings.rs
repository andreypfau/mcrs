use std::collections::VecDeque;
use std::time::Duration;

use crate::{Answer, PingLimits};

#[derive(Clone, Copy, Debug, Default)]
struct SentPing {
    tick: u64,
    at: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct PlayPings {
    limits: PingLimits,
    tick: u64,
    unanswered: VecDeque<SentPing>,
    acknowledged: SentPing,
    previous_tick_wrote: bool,
}

impl PlayPings {
    #[must_use]
    pub fn new(limits: PingLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub fn acknowledged_tick(&self) -> u64 {
        self.acknowledged.tick
    }

    #[must_use]
    pub fn acknowledged_sent_at(&self) -> Duration {
        self.acknowledged.at
    }

    pub fn ping(&mut self, now: Duration) -> Option<i32> {
        (self.newest_tick() != self.tick).then(|| self.send(now))
    }

    pub fn end_tick(&mut self, now: Duration) -> Option<i32> {
        let wrote = self.newest_tick() == self.tick;
        let closing = (self.previous_tick_wrote && !wrote).then(|| self.send(now));
        self.previous_tick_wrote = wrote;
        self.tick = self.tick.saturating_add(1);
        closing
    }

    pub fn answer(&mut self, id: i32) -> Answer {
        let Some(index) = self
            .unanswered
            .iter()
            .position(|sent| ping_id(sent.tick) == id)
        else {
            return Answer::Unknown;
        };
        if let Some(newest) = self.unanswered.drain(..=index).next_back() {
            self.acknowledged = newest;
        }
        if index == 0 {
            Answer::Oldest
        } else {
            Answer::Skipped
        }
    }

    #[must_use]
    pub fn overdue(&self, now: Duration) -> bool {
        self.unanswered.len() > self.limits.max_unanswered
            || self
                .unanswered
                .front()
                .is_some_and(|sent| now.saturating_sub(sent.at) > self.limits.timeout)
    }

    fn newest_tick(&self) -> u64 {
        self.unanswered.back().unwrap_or(&self.acknowledged).tick
    }

    fn send(&mut self, now: Duration) -> i32 {
        self.unanswered.push_back(SentPing {
            tick: self.tick,
            at: now,
        });
        ping_id(self.tick)
    }
}

fn ping_id(tick: u64) -> i32 {
    (tick as u32).cast_signed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn pinged(limits: PingLimits, ticks: u64) -> PlayPings {
        let mut pings = PlayPings::new(limits);
        assert_eq!(
            pings.ping(seconds(0)),
            None,
            "the first play tick sends no ping"
        );
        assert_eq!(pings.end_tick(seconds(0)), None);
        for tick in 1..=ticks {
            assert_eq!(pings.ping(seconds(tick)), Some(ping_id(tick)));
            assert_eq!(pings.ping(seconds(tick)), None, "one ping for each tick");
            assert_eq!(pings.end_tick(seconds(tick)), None);
        }
        pings
    }

    #[test]
    fn answers_advance_the_acknowledged_tick() {
        use Answer::{Oldest, Skipped, Unknown};
        let cases: [(&[i32], &[Answer], u64); 4] = [
            (&[1, 2], &[Oldest, Oldest], 2),
            (&[3, 1], &[Skipped, Unknown], 3),
            (&[99, 1], &[Unknown, Oldest], 1),
            (&[2, 2, 3], &[Skipped, Unknown, Oldest], 3),
        ];
        for (ids, expected, acknowledged) in cases {
            let mut pings = pinged(PingLimits::default(), 3);
            let answers: Vec<Answer> = ids.iter().map(|&id| pings.answer(id)).collect();
            assert_eq!(answers, expected, "{ids:?}");
            assert_eq!(pings.acknowledged_tick(), acknowledged, "{ids:?}");
            assert_eq!(
                pings.acknowledged_sent_at(),
                seconds(acknowledged),
                "{ids:?}"
            );
        }
    }

    #[test]
    fn more_unanswered_pings_than_the_bound_or_a_late_answer_is_overdue() {
        let limits = PingLimits {
            max_unanswered: 2,
            timeout: seconds(10),
        };
        let mut pings = pinged(limits, 2);
        assert!(!pings.overdue(seconds(11)));
        assert!(pings.overdue(seconds(12)));
        assert_eq!(pings.ping(seconds(3)), Some(3));
        assert!(pings.overdue(seconds(3)));
        assert_eq!(pings.answer(2), Answer::Skipped);
        assert!(!pings.overdue(seconds(12)));
    }
}
