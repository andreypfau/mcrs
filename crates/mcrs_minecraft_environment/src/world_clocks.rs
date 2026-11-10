#[cfg(feature = "bevy")]
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_resource::Id;
use serde::{Deserialize, Serialize};

use crate::clock_time::ClockTime;

/// An entry of the registry `world_clock`. It has no fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldClock {}

mcrs_minecraft_registries::resolve_as_is!(WorldClock);

/// Whether the clocks advance at all; the game rule `advance_time` sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct AdvanceTime(pub bool);

impl Default for AdvanceTime {
    fn default() -> Self {
        AdvanceTime(true)
    }
}

/// The state of every clock of the registry, by clock id.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct WorldClocks(Vec<ClockTime>);

impl WorldClocks {
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn get(&self, clock: Id<WorldClock>) -> Option<&ClockTime> {
        self.0.get(clock.index())
    }

    pub fn get_mut(&mut self, clock: Id<WorldClock>) -> Option<&mut ClockTime> {
        self.0.get_mut(clock.index())
    }

    /// Sets the state of `clock`. Clocks below it that held no state start fresh.
    pub fn insert(&mut self, clock: Id<WorldClock>, state: ClockTime) {
        if self.0.len() <= clock.index() {
            self.0.resize(clock.index() + 1, ClockTime::default());
        }
        if let Some(slot) = self.0.get_mut(clock.index()) {
            *slot = state;
        }
    }

    /// Holds one state for each of `count` clocks: a new clock starts fresh, and a clock
    /// past `count` is dropped.
    pub fn reconcile(&mut self, count: usize) {
        self.0.resize(count, ClockTime::default());
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<WorldClock>, &ClockTime)> {
        let len = self.0.len();
        (0..=u16::MAX)
            .zip(&self.0)
            .filter_map(move |(number, state)| Some((Id::checked(number, len)?, state)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<WorldClock>, &mut ClockTime)> {
        let len = self.0.len();
        (0..=u16::MAX)
            .zip(&mut self.0)
            .filter_map(move |(number, state)| Some((Id::checked(number, len)?, state)))
    }
}
