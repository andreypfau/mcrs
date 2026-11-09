use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, Named, Refs, Registry, Resolve, Resolved, registry_value,
};
use mcrs_minecraft_resource::{Id, ResourceLocationBuf};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::WorldClock;
use crate::environment_attribute_keys::EnvironmentAttribute;
use crate::info;
use crate::time_marker::TimeMarker;
use crate::track::{Track, deserialize_tracks, serialize_tracks};

fn positive_period<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    match Option::<i64>::deserialize(d)? {
        Some(period) if period > 0 => u32::try_from(period)
            .map(Some)
            .map_err(|_| D::Error::custom(format!("period_ticks {period} is too large"))),
        Some(period) => Err(D::Error::custom(format!(
            "period_ticks {period} is not positive"
        ))),
        None => Ok(None),
    }
}

/// A timeline: tracks of attributes and time markers, on one clock.
#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct Timeline<M: Refs = Resolved> {
    pub clock: M::Id<WorldClock>,
    #[serde(
        default,
        deserialize_with = "positive_period",
        skip_serializing_if = "Option::is_none"
    )]
    pub period_ticks: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub time_markers: BTreeMap<ResourceLocationBuf, TimeMarker>,
    #[serde(
        default,
        serialize_with = "serialize_tracks",
        deserialize_with = "deserialize_tracks",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub tracks: BTreeMap<EnvironmentAttribute, Track>,
}

impl<M: Refs> Timeline<M> {
    fn check(&self) -> Result<(), String> {
        for (attribute, track) in &self.tracks {
            track
                .check(self.period_ticks)
                .map_err(|message| format!("tracks.{}: {message}", attribute.as_static_str()))?;
        }
        if let Some(period) = self.period_ticks {
            for (name, marker) in &self.time_markers {
                if marker.ticks >= period {
                    return Err(format!(
                        "time_markers.{name}: {} is not less than the period {period}",
                        marker.ticks
                    ));
                }
            }
        }
        Ok(())
    }

    /// The timeline as a client receives it: only the tracks of syncable attributes.
    #[must_use]
    pub fn synced(mut self) -> Self {
        self.tracks.retain(|attribute, _| info(*attribute).syncable);
        self
    }
}

impl<'de> Deserialize<'de> for Timeline<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut timeline = Timeline::deserialize(d)?;
        timeline.check().map_err(D::Error::custom)?;
        let period = timeline.period_ticks;
        for track in timeline.tracks.values_mut() {
            track.set_period(period);
        }
        Ok(timeline)
    }
}

impl Serialize for Timeline<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Timeline::serialize(self, s)
    }
}

impl Timeline {
    /// The registry declaration, with the check that no clock has a time marker twice.
    pub fn declaration() -> DataDeclaration<Self> {
        Declaration::data::<Self>().validate(|registry, registries| {
            let clocks = registries.registry::<WorldClock>();
            check_time_markers(registry, |clock| {
                clocks
                    .and_then(|clocks| clocks.name(clock))
                    .map_or_else(|| format!("#{}", clock.number()), ToString::to_string)
            })
        })
    }
}

/// The load fails when two timelines of one clock name the same time marker.
fn check_time_markers(
    timelines: &Registry<Timeline>,
    clock_name: impl Fn(Id<WorldClock>) -> String,
) -> Vec<(usize, String)> {
    let mut seen: HashMap<(Id<WorldClock>, &ResourceLocationBuf), &ResourceLocationBuf> =
        HashMap::new();
    let mut refused = Vec::new();
    for (at, (_, name, timeline)) in timelines.iter().enumerate() {
        for marker in timeline.time_markers.keys() {
            if let Some(first) = seen.insert((timeline.clock, marker), name) {
                refused.push((
                    at,
                    format!(
                        "the time marker {marker} of the clock {} is defined more than once, \
                         first in the timeline {first}",
                        clock_name(timeline.clock)
                    ),
                ));
            }
        }
    }
    refused
}

/// A timeline as the network carries it: the tracks of attributes a client may not see are
/// left out.
#[derive(Debug, Clone, Resolve)]
pub struct NetworkTimeline<M: Refs = Resolved>(Timeline<M>);

impl Serialize for NetworkTimeline<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for NetworkTimeline<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Timeline::<Named>::deserialize(d).map(|timeline| NetworkTimeline(timeline.synced()))
    }
}

impl From<NetworkTimeline> for Timeline {
    fn from(timeline: NetworkTimeline) -> Self {
        timeline.0
    }
}

impl From<&Timeline> for NetworkTimeline {
    fn from(timeline: &Timeline) -> Self {
        NetworkTimeline(timeline.clone().synced())
    }
}
