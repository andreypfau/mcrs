use std::time::Duration;

use bevy::prelude::*;

use crate::Unsettled;
use crate::stream::Streaming;

/// Tracks how long the terrain has stayed quiet. `Streaming::done` also holds in the gaps between
/// the server's batches, so the terrain counts as quiet only once nothing new arrives and no
/// plugin reports `Unsettled`.
#[derive(Default)]
pub struct Settle {
    quiet_since: Option<Duration>,
    counts: [usize; 3],
}

impl Settle {
    /// Reads the streaming state and the `Unsettled` messages of this frame. Returns how long
    /// the terrain has been quiet, or `None` while it is not.
    pub fn observe(
        &mut self,
        now: Duration,
        streaming: &Streaming,
        unsettled: &mut MessageReader<Unsettled>,
    ) -> Option<Duration> {
        let status = streaming.status();
        let counts = [status.columns, status.sections, status.sections_total];
        let quiet = unsettled.read().count() == 0
            && streaming.done()
            && status.columns > 0
            && counts == self.counts;
        self.counts = counts;
        if quiet {
            Some(now.saturating_sub(*self.quiet_since.get_or_insert(now)))
        } else {
            self.quiet_since = None;
            None
        }
    }

    pub fn restart(&mut self) {
        self.quiet_since = None;
    }
}
