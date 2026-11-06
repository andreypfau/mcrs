//! The probes of a connection whose moves the server checks: a `ping` before and after the
//! changes that the client predicts with, written once the order on the wire is final.

use std::collections::VecDeque;

use bevy_ecs::component::Component;
use mcrs_minecraft_network::Instant;

/// The probes that the server wrote to one connection and that wait for an answer. A
/// connection without this component gets no probes.
#[derive(Component, Debug, Default)]
pub struct Probes {
    next_id: i32,
    /// No predicted change went out after the newest probe, so it can serve as the probe before
    /// the next change.
    fresh: bool,
    sent: VecDeque<SentProbe>,
}

/// A probe that went out, in the order of the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentProbe {
    pub id: i32,
    pub at: Instant,
}

impl Probes {
    /// The probes without an answer, oldest first.
    pub fn sent_mut(&mut self) -> &mut VecDeque<SentProbe> {
        &mut self.sent
    }

    /// A probe goes before a predicted change unless the newest probe already precedes it with
    /// no change between.
    pub(crate) fn needs_probe_before_change(&self) -> bool {
        !self.fresh
    }

    pub(crate) fn changed(&mut self) {
        self.fresh = false;
    }

    /// Records a probe written at `at` and answers its id, unique among the probes without an
    /// answer while fewer than 2^32 of them wait.
    pub(crate) fn probe(&mut self, at: Instant) -> i32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.fresh = true;
        self.sent.push_back(SentProbe { id, at });
        id
    }
}
