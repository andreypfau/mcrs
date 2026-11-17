use std::collections::VecDeque;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
struct Unanswered {
    id: i32,
    sent_at: Duration,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Probes {
    next_id: i32,
    unanswered: VecDeque<Unanswered>,
    sent: u64,
    answered: u64,
    newest_answered_at: Option<Duration>,
}

pub(crate) enum Answer {
    Oldest,
    Skipped,
    Unknown,
}

impl Probes {
    pub(crate) fn send(&mut self, now: Duration) -> i32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.unanswered.push_back(Unanswered { id, sent_at: now });
        self.sent = self.sent.saturating_add(1);
        id
    }

    pub(crate) fn newest(&self) -> Option<u64> {
        self.sent.checked_sub(1)
    }

    pub(crate) fn answered(&self) -> u64 {
        self.answered
    }

    pub(crate) fn newest_answered_at(&self) -> Option<Duration> {
        self.newest_answered_at
    }

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

    pub(crate) fn waits_longer(&self, now: Duration, limit: Duration) -> bool {
        self.unanswered
            .front()
            .is_some_and(|probe| now.saturating_sub(probe.sent_at) > limit)
    }
}
