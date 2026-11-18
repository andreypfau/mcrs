use std::time::Duration;

use bevy::prelude::*;

use crate::Unsettled;
use crate::stream::Streaming;

#[derive(Default)]
pub struct Settle {
    quiet_since: Option<Duration>,
    counts: [usize; 3],
    blockers: Blockers,
}

#[derive(Default, Clone, Copy)]
pub struct Blockers {
    pub unsettled_messages: usize,
    pub streaming_done: bool,
    pub columns: usize,
    pub counts_changed: bool,
    pub queued: usize,
    pub meshing: usize,
    pub uploads_waiting: usize,
}

impl Blockers {
    pub fn is_quiet(&self) -> bool {
        self.unsettled_messages == 0
            && self.streaming_done
            && self.columns > 0
            && !self.counts_changed
    }

    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.unsettled_messages > 0 {
            names.push("unsettled messages");
        }
        if !self.streaming_done {
            names.push("streaming not done");
        }
        if self.columns == 0 {
            names.push("no columns loaded");
        }
        if self.counts_changed {
            names.push("column and section counts still changing");
        }
        names
    }
}

impl Settle {
    pub fn observe(
        &mut self,
        now: Duration,
        streaming: &Streaming,
        unsettled: &mut MessageReader<Unsettled>,
    ) -> Option<Duration> {
        let status = streaming.status();
        let counts = [status.columns, status.sections, status.sections_total];
        let blockers = Blockers {
            unsettled_messages: unsettled.read().count(),
            streaming_done: streaming.done(),
            columns: status.columns,
            counts_changed: counts != self.counts,
            queued: status.queued,
            meshing: status.meshing,
            uploads_waiting: status.uploads_waiting,
        };
        let quiet = blockers.is_quiet();
        self.blockers = blockers;
        self.counts = counts;
        if quiet {
            Some(now.saturating_sub(*self.quiet_since.get_or_insert(now)))
        } else {
            self.quiet_since = None;
            None
        }
    }

    pub fn blockers(&self) -> Blockers {
        self.blockers
    }

    pub fn restart(&mut self) {
        self.quiet_since = None;
    }
}
