#[cfg(feature = "bevy")]
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_resource::{Id, ResourceLocationBuf};

use crate::Timeline;
use crate::world_clocks::WorldClock;

/// A time marker of a clock, with the period of the timeline that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockTimeMarker {
    pub ticks: u32,
    pub period_ticks: Option<u32>,
    pub show_in_commands: bool,
}

impl ClockTimeMarker {
    /// Whether the clock stands on the marker at `total_ticks`.
    #[must_use]
    pub fn occurs_at(&self, total_ticks: i64) -> bool {
        let ticks = i64::from(self.ticks);
        match self.period_ticks.map(i64::from) {
            Some(period) => total_ticks % period == ticks,
            None => total_ticks == ticks,
        }
    }

    /// The total ticks at which the marker next occurs, for a marker that does not occur at
    /// `total_ticks`. Without a period this can lie before `total_ticks`.
    #[must_use]
    pub fn next_after(&self, total_ticks: i64) -> i64 {
        let ticks = i64::from(self.ticks);
        match self.period_ticks.map(i64::from) {
            Some(period) => {
                let delta = ticks - total_ticks % period;
                if delta > 0 {
                    total_ticks.saturating_add(delta)
                } else {
                    total_ticks.saturating_add(period + delta)
                }
            }
            None => ticks,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    clock: Id<WorldClock>,
    name: ResourceLocationBuf,
    marker: ClockTimeMarker,
}

/// The time markers of every clock, from the timelines that run on it.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct ClockTimeMarkers(Vec<Entry>);

impl ClockTimeMarkers {
    #[must_use]
    pub fn new(timelines: &[Timeline]) -> Self {
        ClockTimeMarkers(
            timelines
                .iter()
                .flat_map(|timeline| {
                    timeline.time_markers.iter().map(|(name, marker)| Entry {
                        clock: timeline.clock,
                        name: name.clone(),
                        marker: ClockTimeMarker {
                            ticks: marker.ticks,
                            period_ticks: timeline.period_ticks,
                            show_in_commands: marker.show_in_commands,
                        },
                    })
                })
                .collect(),
        )
    }

    pub fn of_clock(
        &self,
        clock: Id<WorldClock>,
    ) -> impl Iterator<Item = (&ResourceLocationBuf, &ClockTimeMarker)> {
        self.0
            .iter()
            .filter(move |entry| entry.clock == clock)
            .map(|entry| (&entry.name, &entry.marker))
    }
}
