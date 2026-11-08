use std::collections::VecDeque;
use std::time::Duration;

/// A probe without an answer, in the order of the wire.
#[derive(Clone, Copy, Debug)]
struct Unanswered {
    id: i32,
    sent_at: Duration,
}

/// The probes of one connection. A probe is numbered by its place in the order of the wire;
/// the probes below `answered` have an answer.
#[derive(Clone, Debug, Default)]
pub(crate) struct Probes {
    next_id: i32,
    unanswered: VecDeque<Unanswered>,
    sent: u64,
    answered: u64,
    newest_answered_at: Option<Duration>,
}

/// What an answer matched.
pub(crate) enum Answer {
    Oldest,
    /// A newer probe; the older ones count as answered too.
    Skipped,
    Unknown,
}

impl Probes {
    /// Records a probe sent at `now` and answers its id, unique among the probes without an
    /// answer while fewer than 2^32 of them wait.
    pub(crate) fn send(&mut self, now: Duration) -> i32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.unanswered.push_back(Unanswered { id, sent_at: now });
        self.sent = self.sent.saturating_add(1);
        id
    }

    /// The number of the newest probe, if any was sent.
    pub(crate) fn newest(&self) -> Option<u64> {
        self.sent.checked_sub(1)
    }

    pub(crate) fn answered(&self) -> u64 {
        self.answered
    }

    pub(crate) fn newest_answered_at(&self) -> Option<Duration> {
        self.newest_answered_at
    }

    /// Matches an answer with its probe. The ids of the probes without an answer follow each
    /// other, so the place of an id is its distance from the oldest one.
    pub(crate) fn answer(&mut self, id: i32) -> Answer {
        let Some(oldest) = self.unanswered.front() else {
            return Answer::Unknown;
        };
        let index = id.wrapping_sub(oldest.id).cast_unsigned() as usize;
        if index >= self.unanswered.len() {
            return Answer::Unknown;
        }
        for probe in self.unanswered.drain(..=index) {
            self.answered = self.answered.saturating_add(1);
            self.newest_answered_at = Some(probe.sent_at);
        }
        if index == 0 {
            Answer::Oldest
        } else {
            Answer::Skipped
        }
    }

    /// Whether the oldest probe without an answer waits longer than `limit` at `now`.
    pub(crate) fn waits_longer(&self, now: Duration, limit: Duration) -> bool {
        self.unanswered
            .front()
            .is_some_and(|probe| now.saturating_sub(probe.sent_at) > limit)
    }
}
